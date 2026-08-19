//! **The Now playing place** — the current song, and nothing competing with it.
//!
//! One large source-bounded cover and the track-led placard.
//! The run is not another version of an album or playlist page and is not
//! drawn here. A quiet provenance link is the road to the real source page:
//! the originating playlist when one still exists, the current unsaved list
//! when the run is one, otherwise the sounding track's album. The
//! persistent bottom bar remains the only transport.

use iced::widget::{Space, button, column, container, image as iced_image, row, stack, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::field;
use crate::player::PlayerState;
use crate::theme;

/// The source page Now playing can quietly lead to.
#[derive(Debug, Clone)]
pub(crate) enum Source {
    /// A run reified from a playlist file that still exists.
    Playlist { id: u64, name: String },
    /// The current run materialized as an unsaved playlist.
    Queue { name: String },
    /// The record the sounding track resolves into.
    Album { id: u64, name: String },
}

impl Source {
    /// The shared navigation used by both the footer and the persistent bar.
    pub(crate) fn open_message(&self) -> Message {
        match self {
            Self::Playlist { id, .. } => Message::OpenPlaylist(*id),
            Self::Queue { .. } => Message::ShowQueue,
            Self::Album { id, .. } => Message::OpenAlbum(*id),
        }
    }
}

/// **The work size the desktop composition was tuned at** — the old
/// `NOW_PLAYING_MAX`, kept as the *reference* it always secretly was after step
/// A2 deleted it as a *ceiling*.
///
/// It is the denominator of [`kiosk_scale`] and nothing else. A2's finding was
/// that 720 is a lie when it bounds a decode; it is an honest number when it
/// names the size this surface's measures were chosen against, which is what
/// makes the scale's floor of `1.0` a promise that every window at or below it
/// is pixel-identical to the build before A4.
const FAR_FIELD_REF: f32 = 720.0;

/// The ceiling on [`kiosk_scale`] — doc 12 §11.2's `2.5`. It stops a very large
/// panel producing measures that are absurd at 60 cm on that same panel.
const KIOSK_SCALE_MAX: f32 = 2.5;

/// **How much larger than the desktop composition this window is** — doc 12
/// §11.2's `kiosk_scale`, `1.0` at every window this product was designed at
/// and up to [`KIOSK_SCALE_MAX`] beyond it.
///
/// # It is keyed to the height, and that is deliberate
///
/// §11.2 prints `kiosk_scale(edge)`; §5.5a prints `kiosk_scale(by_height)` and
/// explains why the second is the one that can be built: **`edge` depends on
/// `run_w`, and `run_w` is what this scales.** Keying the scale to the work's
/// resolved size would make the run's width depend on the record's width, which
/// depends on the run's width, and the fixed point would have to be iterated or
/// fudged. `by_height` — the height-bound candidate for the work — is the same
/// quantity one term earlier, and it does not depend on the run at all.
///
/// So this takes the **window's height**, computes `art_edge`'s own height term
/// from it, and hands back a ratio. One honest substitution, named here so
/// nobody later "fixes" it into a cycle.
#[must_use]
fn kiosk_scale(height: f32) -> f32 {
    let by_height = height - 2.0 * theme::HANG - BELOW;
    (by_height / FAR_FIELD_REF).clamp(1.0, KIOSK_SCALE_MAX)
}

/// **The run column's width in a body `width` × `height` px**, or `0` when the
/// run is not standing beside the record.
///
/// Two conditions for the zero, and since the `Run` word went **both** are facts
/// rather than preferences: there is no run at all, or the body is below
/// [`theme::SPLIT_FLOOR`] and the two columns have re-stacked into one, where
/// the run takes the whole measure and the record becomes its head.
///
/// # It grows with the window — doc 12 step A4, and the owner's own report
///
/// The owner, 2026-08-10: *"at full screen the now playing page looks odd
/// because the playlist hugs right and the art hugs left"*. [`theme::RUN_MEASURE`]
/// **440** is half of [`theme::LIST_MEASURE`], derived for a 1280–1920 window,
/// and it stayed 440 at every size — so at 2560 the run was a 440 px ribbon
/// with **1171 px of bare field between it and the sleeve**, measured off the
/// frames in `docs/design/impl/one-list-drawn-once/`. This is the *right* edge of his
/// sentence; [`view`]'s centring is the left one, and neither alone is the fix.
///
/// So the measure is scaled by [`kiosk_scale`] — 440 up to a
/// [`FAR_FIELD_REF`] work, **503** at 1920 × 1080, **723** at 2560 × 1440, and
/// **1100** at 4K where the scale reaches its ceiling.
///
/// The 1920 figure is 440 in doc 12 §5.5a's table and it is **503** here;
/// `the_run_grows_with_the_panel_and_the_gap_does_not` carries the reconciliation,
/// which is the same stale `below` the table already corrects twice for other
/// rows. The work at that size does not change either way.
///
/// # The cap, which is the floor `SPLIT_FLOOR` guarantees, held at every size
///
/// [`theme::SPLIT_FLOOR`] is *derived* as `ART_MIN + 2·HANG + RUN_MEASURE +
/// GAP_XL` — the narrowest body in which the record can be [`theme::ART_MIN`]
/// **and** the run can be `RUN_MEASURE`. A run that grows without a cap breaks
/// the half of that guarantee it does not own: a tall, narrow window (784 × 4000
/// is above the floor) would scale the run to 1100 and leave the record
/// *negative*.
///
/// So the scaled measure is capped at whatever leaves the record its floor. The
/// cap can never bite below `RUN_MEASURE`, because that is what `SPLIT_FLOOR`
/// being derived from these four terms *means* — and at the floor itself the cap
/// is exactly 440, which is why the record does not lurch across it.
#[must_use]
pub(crate) fn run_w(width: f32, height: f32, run: bool) -> f32 {
    if !(run && width >= theme::SPLIT_FLOOR) {
        return 0.0;
    }
    // Spelled `.max(RUN_MEASURE)` rather than left to the arithmetic so this
    // cannot become a `clamp` whose low exceeds its high and panics — the
    // derivation above says it cannot, and a floor costs nothing to state.
    let cap = (width - 2.0 * theme::HANG - theme::GAP_XL - theme::ART_MIN).max(theme::RUN_MEASURE);
    (theme::RUN_MEASURE * kiosk_scale(height)).clamp(theme::RUN_MEASURE, cap)
}

/// **The edge the record is actually drawn at**, whichever composition the
/// body's width has put it in.
///
/// Above [`theme::SPLIT_FLOOR`] it is [`art_edge`]'s answer, with the run's
/// column taken off the width when the run is standing. Below it the columns
/// have re-stacked and the record is the run's **head block** at
/// [`theme::ART_MIN`] — the size at which a cover stops being a subject is the
/// size the head gives it, because what is left worth doing at that width is
/// the list (doc 12 §5.5a).
///
/// `source` is the shortest edge of the decode being drawn, and it bounds the
/// answer in **both** compositions: a 120 px cover is 120 px in the head block
/// as surely as it is in the record column, because the refusal is about the
/// file rather than about the layout.
///
/// Stating it in one function is what keeps the surface monotonic across the
/// floor: the record does not lurch when the two columns become one.
#[must_use]
pub(crate) fn record_edge(width: f32, height: f32, run: bool, source: f32) -> f32 {
    if run && width < theme::SPLIT_FLOOR {
        theme::ART_MIN.min(source)
    } else {
        art_edge(width, height, run_w(width, height, run), source)
    }
}

/// **The artwork's edge**, derived from the viewport, the column beside it,
/// and **the source's own pixels**.
///
/// The whole of what makes the kiosk mode this surface at a larger size: the
/// work takes the room it is given, bounded below so it never stops being the
/// subject and above by the one thing that may honestly bound it. The height
/// term is what stops a wide, short window pushing the placard off the bottom
/// — a now-playing screen that has scrolled away from what is playing is not
/// one.
///
/// `run_w` is [`run_w`]'s answer, and it is subtracted from the *width* term
/// alone: the run's head sits in the run's own column, so it costs the record
/// no height (doc 12 §5.5a). **The run costs the record nothing wherever the
/// record is height-bound**, which is every window above the narrowest one this
/// product draws — `the_run_costs_the_record_nothing_where_it_is_height_bound`
/// is that claim swept.
///
/// # The third term, and the constant it replaced
///
/// `source` is `min(w, h)` of the decode this surface is actually drawing —
/// [`crate::app::Hero::px`] once the hero has landed, the thumbnail's own edge
/// before it, and **infinite** for a record with no art at all, because the
/// deterministic gradient placeholder has no resolution and *larger than its
/// source* is not a predicate that applies to it.
///
/// It replaces `NOW_PLAYING_MAX` **720**, which was deleted with step A2. That
/// constant was a fixed number standing in for a fact about the decode, and it
/// was wrong in both directions at once: it let a 320 px thumbnail be drawn at
/// 2.25× on any panel 1080 px tall or better — *no artwork is ever drawn
/// larger than its source*, false in the one place nobody had a test for
/// (ADR-0029 §Context 2) — while capping a 4K panel's work at 720 px in a
/// 3744 px body, which is doc 12 §5.5's *"postage stamp in a void"* and is the
/// thing the owner saw. **A fact about the file cannot be spelled as a
/// constant in a view**, and this is the general form of that.
///
/// # Where the clamps sit, and why that order
///
/// ```text
/// min(by_width, by_height).max(ART_MIN).min(source)
/// ```
///
/// The floor is applied **before** the source ceiling, and the ceiling wins.
/// Doc 12 §5.2 prints `min(…, hero_px).max(ART_MIN)`, which disagrees with the
/// test printed six lines under it — at `hero_px` 120 that expression is 240,
/// and `art_edge(side, side, 120) <= 120` fails. The test is right and the
/// formula is not: [`theme::ART_MIN`] is a **design** floor, saying a work
/// this small has stopped being a subject, and `source` is a **fact**, saying
/// there are no more pixels. A fact outranks a floor, story S7 asks for the
/// small cover *"drawn at its own pixel size, centred, never scaled up"*, and
/// the field is what makes that composed rather than broken.
#[must_use]
pub(crate) fn art_edge(width: f32, height: f32, run_w: f32, source: f32) -> f32 {
    art_edge_with_below(width, height, run_w, source, BELOW)
}

fn art_edge_with_below(width: f32, height: f32, run_w: f32, source: f32, below: f32) -> f32 {
    let beside = if run_w > 0.0 {
        run_w + theme::GAP_XL
    } else {
        0.0
    };
    let by_width = width - 2.0 * theme::HANG - beside;
    let by_height = height - 2.0 * theme::HANG - below;
    by_width
        .min(by_height)
        .max(theme::ART_MIN)
        .min(source)
        .max(0.0)
}

/// Draw the current song at the size the viewport and its source permit.
#[derive(Clone, Copy)]
pub(crate) struct Visual<'a> {
    pub(crate) rotation: crate::jewel_case::Rotation,
    pub(crate) foreground: crate::visualizer::Foreground,
    pub(crate) mode: crate::visualizer::Mode,
    /// Present only while the independently selected visualization is visible.
    pub(crate) audio: Option<&'a baz_core::engine::VisualizationFrame>,
    pub(crate) history: &'a crate::visualizer::History,
    /// The sounding library file and its durable membership reading. Unknown
    /// external files carry no inert heart.
    pub(crate) favourite: Option<(&'a std::path::Path, bool)>,
}

pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    width: f32,
    height: f32,
    source: Option<Source>,
    visual: Visual<'_>,
    fact: Option<&String>,
) -> Element<'a, Message> {
    let Some(now) = player.now_playing() else {
        // **A start in flight is not silence.** `Resume` on the Home place
        // navigates here in the same press that asks the engine to begin
        // (`App::resume_the_run`), and the engine's `TrackStarted` is a frame
        // or two behind the press — so for those frames there is a record on
        // its way and no record yet to draw.
        //
        // The place stays bare rather than announcing silence it is about to
        // contradict. A sentence that appears and vanishes is *read*, and a
        // statement of silence is the one thing this surface must never make
        // while something is starting; a blank that fills is not read at all.
        // The condition is the engine's own — a transport command awaiting its
        // confirming event — so nothing here has to know which press sent it.
        if player.transport_pending() {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        }
        let room = theme::active();
        return container(
            text("Nothing playing")
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .color(room.paper_dim),
        )
        .center(Length::Fill)
        .into();
    };
    // An album source footer already names this exact album. A playlist or
    // assembled run can cross records, so its current track keeps the album
    // line where that distinction is useful.
    let show_album = show_album_line(now.album.as_deref(), source.as_ref());

    // **The record's own colours**, read off the hero. They light the artist
    // line and the visualiser's bars, so the page is demonstrably about *this*
    // record rather than tinted at random.
    let hues = now
        .album_id
        .and_then(|id| shelf.hero(id))
        .and_then(|hero| hero.field);

    // **One composition, whether or not there is an album object.**
    //
    // The two branches used to be different layouts, which is why the
    // objectless state kept getting designed last. Here the object is simply
    // absent: the marquee is anchored to the place's own corner, not to the
    // artwork, so it stands in exactly the same spot with a cover, with a
    // jewel case, and with nothing at all.
    let draws_art = visual.foreground.draws_art();
    let work = draws_art.then(|| work(shelf, Some(now)));
    // **The case is an object; the cover is a picture**, and the source's own
    // pixels bound only one of them.
    //
    // The owner: *"in some cases the spinning CD is quite small — I think this
    // might be due to the artwork size? we probably want to scale the artwork
    // to a consistent size."* It was: a 300 px insert drew a 300 px case, and
    // the next record's 1000 px one drew a 792 px case, so the CD changed size
    // between tracks for a reason nothing on screen explained.
    //
    // The never-upscale rule stands where it means something. A **plain
    // cover** is the file, drawn: enlarging it past its own pixels is baz
    // claiming detail the file does not have, which is ADR-0029 §Context 2 and
    // story S7's *"drawn at its own pixel size, centred, never scaled up"*. A
    // **jewel case** is not the file — it is a rendered object whose front is
    // a *texture*, and a real CD does not come in a smaller box because its
    // insert was printed at lower resolution. A low-res cover there is a
    // softer print on a case of the ordinary size, which is the truthful
    // reading and the one the owner is asking for.
    let bound = if visual.foreground.draws_case() {
        f32::INFINITY
    } else {
        work.as_ref().map_or(0.0, |work| work.source)
    };
    let edge = work
        .as_ref()
        .map_or(0.0, |_| marquee_edge(width, height, bound));
    let t = work
        .as_ref()
        .map_or(1.0, |work| work.dissolve_at(edge, width, height, false));
    let field = match &work {
        Some(work) => field_layer(
            work.from
                .as_ref()
                .and_then(|&(_, _, field)| field)
                .filter(|_| t < 1.0),
            work.field,
            t,
        ),
        // With no object there is no dissolve to ride, so the wash is simply
        // the record's own field, settled.
        None => field_layer(None, hues, 1.0),
    };
    let insert = rear_insert(shelf, now);
    let object: Element<'a, Message> = match &work {
        Some(work) => {
            let cover = plain_cover(work, t, edge, insert.album_id);
            let case = sleeve(work, t, edge, visual.rotation, &insert);
            crate::visualizer::foreground(visual.foreground, edge, cover, case)
        }
        // No object, and no column reserved for one: the marquee takes the
        // whole measure, which is what the objectless mode was always for.
        None => Space::new().width(0.0).height(Length::Fill).into(),
    };

    let body = stage(
        object,
        marquee(
            now,
            show_album,
            marquee_measure(width, height),
            visual.favourite,
            fact.map(String::as_str),
            hues,
        ),
        source,
    );

    // **The field and the spectrum are not here.** They are the window's
    // backdrop now, drawn by `App::view` behind the app bar as well as behind
    // this page — see [`backdrop`]. What stays is the page itself, which is
    // transparent over them.
    let _ = field;
    body
}

/// **The record's wash and its spectrum, sized to the whole window.**
///
/// The owner, of chromeless mode: *"make sure the top bar is transparent
/// essentially… the visualiser and background colour etc should be showing
/// where it is currently black."*
///
/// The bar had been transparent for a while and read black anyway, because
/// these two layers lived *inside* the page — and the page is the window less
/// the bar and the lane. There was nothing under the glass but the window's
/// own ground.
///
/// Stacking the bar over the page instead was tried and was wrong: it moves
/// every place up under the bar, and the lane, which has no margin of its own,
/// ran off the top of the window. The layers that want to be behind the bar
/// are these two, so it is these two that leave the page. The lane keeps its
/// own ground and stays readable over them.
pub(crate) fn backdrop<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    visual: Visual<'_>,
    window: iced::Size,
) -> Element<'a, Message> {
    let Some(now) = player.now_playing() else {
        return field_layer(None, None, 1.0);
    };
    let hues = now
        .album_id
        .and_then(|id| shelf.hero(id))
        .and_then(|hero| hero.field);
    let work = visual
        .foreground
        .draws_art()
        .then(|| work(shelf, Some(now)));
    let t = work.as_ref().map_or(1.0, |work| {
        let edge = marquee_edge(window.width, window.height, f32::INFINITY);
        work.dissolve_at(edge, window.width, window.height, false)
    });
    let field = match &work {
        Some(work) => field_layer(
            work.from
                .as_ref()
                .and_then(|&(_, _, field)| field)
                .filter(|_| t < 1.0),
            work.field,
            t,
        ),
        None => field_layer(None, hues, 1.0),
    };
    let spectrum: Element<'static, Message> = if let Some(audio) = visual.audio {
        crate::visualizer::background(
            visual.mode,
            audio,
            visual.history,
            window.width,
            window.height,
            hues,
        )
    } else {
        Space::new().width(Length::Fill).height(Length::Fill).into()
    };
    stack![field, spectrum].into()
}

/// Whether the album line is worth drawing: an album source footer already
/// names this exact record, so the line would be the same string twice. A
/// playlist or an assembled run can cross records, and there it is the fact
/// that says which one you are on.
fn show_album_line(album: Option<&str>, source: Option<&Source>) -> bool {
    album.is_some() && !matches!(source, Some(Source::Album { .. }))
}

/// The full-width source footer reserved at the bottom of the place.
const SOURCE_CARD_H: f32 = 108.0;

/// What the old centred composition reserved under the work. Kept because
/// `art_edge` and its tests still speak in it; the marquee sizes its object
/// with [`marquee_edge`] instead.
const BELOW: f32 = theme::GAP_XL
    + theme::LINE_HEADING
    + theme::GAP_XS
    + theme::LINE_DISPLAY
    + theme::GAP_XS
    + theme::LINE_BODY;

/// **The place's own margin** — where the marquee's left edge stands.
///
/// [`theme::HANG`] and a half. The type is the largest thing on this surface
/// and a 40 px margin under a 64 px line reads as a crop rather than a margin;
/// this is the one place in the product that needs more air than the standard
/// gutter, and it is stated here rather than by adding a token nothing else
/// would use.
const MARGIN: f32 = 1.5 * theme::HANG;

/// **How wide the title may run.**
///
/// The body less both margins, capped: at 3840 px a title set across the whole
/// window would be a line no eye tracks. The cap is [`theme::LIST_MEASURE`] and
/// a quarter — wider than a reading measure, because this is one line of
/// display type rather than a paragraph, and narrow enough to stay a line.
fn marquee_measure(width: f32, height: f32) -> f32 {
    // **What the object's column leaves**, and the column is what the stage
    // reserves rather than what this record's cover happens to need.
    //
    // That distinction is the whole of `the_marquee_is_anchored_to_the_place`:
    // the measure is a function of the window and of nothing else — not of the
    // artwork's size, not of its shape, not of whether there is any. Measuring
    // the *drawn* edge instead would reflow the title every time a cover
    // arrived at a different resolution, or the foreground mode was cycled,
    // and a line that rewraps when you change how the sleeve is drawn is a
    // line that belongs to the sleeve.
    //
    // Floored at [`MARQUEE_MIN_W`], and still capped: a title set across a 4K
    // window is a line no eye tracks.
    // The window's own measure, capped — the object takes no width from it
    // now that the two overlap, so this is once again a function of the window
    // and of nothing else.
    let _ = height;
    (width - 2.0 * MARGIN).clamp(1.0, theme::LIST_MEASURE * 1.25)
}

/// **The column the stage keeps for the album**, whatever is drawn in it.
///
/// [`marquee_edge`] is this bounded by the source's own pixels; a cover with
/// fewer of them is drawn smaller and centred in the same column, rather than
/// letting the title spread into space that will be taken back by the next
/// record.
fn object_column(width: f32, height: f32) -> f32 {
    let (across, down) = object_region(width, height);
    across.min(down).max(1.0)
}

/// **How big the album object is** in the marquee composition.
///
/// # Twice too small, and the second time is the one that mattered
///
/// The first draft took `room * 0.34`. Told it was tiny, I made it `0.58` —
/// and was told again: *"can you make the now playing album bigger though…
/// it's just very small and cramped up into the corner."*
///
/// A second telling means the *behaviour* is wrong, not the number, and the
/// number was never the mistake. **A share is a guess.** The object has a
/// region — the stage, less the source band, less the page's padding, less the
/// gap to the marquee, less the marquee — and that region was sitting there
/// being ignored while a fraction of something else decided the size. At
/// 1920 × 1080 the share drew 451 px into a region 660 px tall, so a fifth of
/// the room the composition had was empty on purpose and nobody could say why.
///
/// So it is the region. `min(region_w, region_h)`, floored, and bounded by the
/// source — the same shape as [`art_edge`], which is what the centred
/// composition used before this page was rebuilt, and it was right.
///
/// # What a jewel case does to this
///
/// Worth knowing when reading a screenshot: the case is a **projected** object
/// that yaws continuously inside its box, so what reaches the glass is
/// narrower than `edge` and its apparent width breathes as it turns. A plain
/// cover fills the box exactly. The box is the honest number for both, and it
/// is why a case that measured 215 px on screen had a 290 px box behind it.
///
/// # The one rule that does not move
///
/// Never larger than the source's own pixels. That is a fact about the file
/// rather than a preference about the layout: baz does not upscale a cover, at
/// any size, in any composition.
fn marquee_edge(width: f32, height: f32, source: f32) -> f32 {
    // The reserved column, bounded by the source's own pixels. No design floor
    // here: [`theme::CONTINUE_SLEEVE`] used to hold one, and on a stage too
    // narrow to seat both the object and the title's minimum it forced a
    // sleeve that had nowhere to be. The title's floor is the one that binds,
    // and the object gives way to it — see [`object_region`].
    object_column(width, height).min(source).max(1.0)
}

/// **The room the object actually has**, in the stage's own terms.
///
/// # The height term was subtracting something that is not above it
///
/// This read `height − footer − padding − gap − MARQUEE_BLOCK`, as though the
/// object sat *on top of* the marquee. It does not: the marquee is anchored to
/// the place's bottom-**left** corner and the object stands to its right, so
/// the two share the vertical rather than dividing it. That cost the object
/// 130 px of height it already had — on top of the 0.78 of its box the jewel
/// case was filling — and two independent shrinkings is how the art stayed
/// small through two goes at the number.
///
/// So the height is the stage's, and the **width** carries the relationship
/// instead: the object takes what is left after the title's floor. A narrow
/// window shrinks the sleeve rather than crushing the title, which is the
/// right way round — a cover has a smallest useful size and a line of type has
/// a smallest useful measure, and the type's is the harder floor.
///
/// Spelled out rather than folded into [`marquee_edge`] so the subtractions can
/// be read against `view`'s own `row!`: the same page in two notations, and a
/// term added to one has to be added to the other.
fn object_region(width: f32, height: f32) -> (f32, f32) {
    let across = width - 2.0 * MARGIN; // the stage's padding, both sides
    let down = height
        - SOURCE_CARD_H       // the footer band, reserved below the stage
        - MARGIN              // the stage's padding, top
        - theme::GAP_XL; // the stage's padding, bottom
    (across.max(1.0), down.max(1.0))
}

/// **The title ladder** — three rungs, chosen by how much there is to set.
///
/// A continuous fit would land anywhere; three rungs make each step a visible
/// decision. The thresholds are characters rather than measured pixels because
/// the measure changes with the window and the *decision* should not: a title
/// that is set at the top rung on a laptop is set at the top rung on a
/// monitor, and simply wraps sooner.
fn marquee_type(title: &str) -> (f32, f32) {
    match title.chars().count() {
        0..=34 => (theme::SIZE_MARQUEE, theme::LEADING_MARQUEE),
        35..=90 => (theme::SIZE_DISPLAY, theme::LEADING_DISPLAY),
        _ => (theme::SIZE_HERO, theme::LEADING_HERO),
    }
}

/// **The marquee**: who, the work, and what it is on — anchored to the place's
/// bottom-left corner.
///
/// The owner picked this composition on 2026-08-18 from three drawn against
/// all three foreground modes. What it answers, after two attempts that did
/// not: the block **floated** because it was centred, and centring relates a
/// thing to the room rather than to anything in it. Here every line starts on
/// one vertical, and that vertical is the place's own margin — the one edge
/// that cannot move when the artwork changes size, changes shape, or is not
/// there at all.
///
/// And it **expands**. There is no ellipsis on this surface: the title wraps
/// inside a measure far wider than the artwork ever was, and steps down the
/// ladder as it grows. 647 of the owner's 8 602 titles are longer than the
/// measure the old composition gave them.
fn marquee<'a>(
    now: &'a crate::player::NowPlaying,
    show_album: bool,
    measure: f32,
    favourite: Option<(&std::path::Path, bool)>,
    fact: Option<&str>,
    hues: Option<field::Field>,
) -> Element<'a, Message> {
    let room = theme::active();
    // The artist line takes the record's own colour where there is one. It is
    // the smallest text in the composition and the only coloured thing in it,
    // which is what makes a 10 px line hold its own under a 64 px one.
    let accent = hues.map_or(room.paper_faint, |field| field.inks(room)[1]);
    let (size, leading) = marquee_type(&now.title);
    let mut block = column![
        text(theme::tracked(
            &now.artist_line().unwrap_or_default().to_uppercase()
        ))
        .size(theme::SIZE_HEADING)
        .line_height(theme::LEADING_HEADING)
        .font(theme::MEDIUM)
        .color(accent),
        row![
            container(
                // **The serif italic, for a track's title** — see
                // `theme::WORK_TITLE`. This page draws one work and the work
                // is a track.
                text(now.title.clone())
                    .size(size)
                    .line_height(leading)
                    .font(theme::WORK_TITLE)
                    .color(room.paper)
            )
            .max_width(measure - theme::STEPPER_HIT - theme::GAP_MD),
            match favourite {
                Some((path, selected)) => crate::views::page::favourite_slot(path, selected),
                None => Space::new().width(Length::Fixed(0.0)).into(),
            },
        ]
        .spacing(theme::GAP_MD)
        // **Level with the title, not hung above it.** `Start` put a 32 px
        // heart against the top of a line box up to 80 px tall, so on the
        // common one-line title it floated a clear 30 px over the cap height
        // with nothing beside it — a mark about the title, drawn as though it
        // were about the air. Centred, it stands level with the words it
        // belongs to, which is the same reading the lane's destination rows
        // are built on.
        .align_y(iced::Alignment::Center),
    ]
    .spacing(theme::GAP_LG)
    .width(Length::Fixed(measure));
    if show_album && let Some(album) = &now.album {
        block = block.push(
            text(album.clone())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .color(room.paper_dim),
        );
    }
    if let Some(fact) = fact {
        block = block.push(
            button(
                text(fact.to_owned())
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .color(room.paper_faint),
            )
            .padding(0)
            .style(move |_theme, _status| button::Style {
                text_color: room.paper_faint,
                ..button::Style::default()
            })
            .on_press(Message::AdvanceFact),
        );
    }
    container(block)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Left)
        .into()
}

/// What the case's printing needs to know about the record.
///
/// It used to gather the edition's whole track list as well, for the inlay
/// printed on the back. Both flat faces draw the front now, so nothing reads
/// that list — and gathering it meant resolving the edition and cloning every
/// title on each record change, for an image no one saw.
fn rear_insert(_shelf: &Shelf, now: &crate::player::NowPlaying) -> crate::jewel_case::Insert {
    crate::jewel_case::Insert {
        album_id: now.album_id.unwrap_or_default(),
        title: now.album.clone().unwrap_or_else(|| now.title.clone()),
        artist: now.artist_line().unwrap_or_default().to_owned(),
    }
}

/// **What this surface has to draw of a record, and the number that bounds
/// it** — the two answers that must be taken together, so they are taken in
/// one place.
///
/// Three cases, in preference order:
///
/// 1. **The hero** ([`art::load_hero`](crate::art::load_hero), up to 1024 px),
///    bounded by its own decoded size. This is the ordinary case and the one
///    the composition is designed around.
/// 2. **The thumbnail** (up to [`crate::art::THUMB_PX`] 320), bounded by *its*
///    own decoded size, for the frames between arriving at the place and the
///    hero landing. `load_thumb` downscales only, so that bound is a true one
///    — a 120 px cover measures 120 here — and the refusal therefore holds on
///    every frame rather than only on the settled ones.
/// 3. **No art**: the wall's own deterministic gradient, and no bound at all.
///    A gradient has no resolution, so it fills whatever the viewport allows
///    and the placard under it does not move (story S7's third criterion).
///
/// The visible consequence of (2) is that a record whose hero is still
/// decoding shows a small sleeve that grows once. That is the honest reading
/// and it is bounded to a frame or two by
/// [`Shelf::request_hero`](crate::app::Shelf::request_hero), which asks for
/// the hero the moment the engine names a record rather than when this place
/// is opened.
fn work(shelf: &Shelf, now: Option<&crate::player::NowPlaying>) -> Work {
    let showing = shelf.showing();
    // **The committed hero, which is not always the sounding record's.** While
    // a new record's decode is in flight there is no answer for it yet, so the
    // shell holds the picture it has and this draws that — see
    // [`crate::app::Shelf::settle_art`] rule 2. The placard above has already
    // changed; the cover follows when there is a cover to follow with, which is
    // a few tens of milliseconds and is the whole reason the dissolve can be a
    // dissolve rather than a fade to nothing.
    if let Some(hero) = showing.hero {
        return Work {
            handle: Some(hero.handle.clone()),
            back: hero.back.clone(),
            source: hero.px,
            field: hero.field,
            from: showing
                .from
                .map(|from| (from.handle.clone(), from.px, from.field)),
            t: showing.t,
        };
    }
    let Some(id) = now.and_then(|now| now.album_id) else {
        return Work::bare();
    };
    match (shelf.thumb(id), shelf.thumb_edge(id)) {
        (Some(handle), Some(px)) => Work {
            handle: Some(handle.clone()),
            back: None,
            source: px,
            field: None,
            from: None,
            t: 1.0,
        },
        _ => Work::bare(),
    }
}

/// **What this surface draws of the record** — the picture, the number that
/// bounds it, the field derived from the same decode, and, while the record is
/// changing, the picture it is dissolving away from.
///
/// One value because the field is a *reading of the cover*: two functions
/// answering separately could put one record's room behind another record's
/// sleeve for a frame, which is precisely the seam ADR-0020's third amendment
/// exists to avoid putting into time.
struct Work {
    /// The picture, or `None` for the wall's deterministic gradient.
    handle: Option<iced_image::Handle>,
    /// A real rear cover, when the record carries one.
    back: Option<iced_image::Handle>,
    /// `min(w, h)` of the decode being drawn — [`art_edge`]'s third term.
    source: f32,
    /// The field derived from that same decode. Read from the **hero** alone:
    /// 320 px is enough pixels for a palette, but a field that changed colour
    /// when the hero replaced the thumbnail would be the room flickering on
    /// every record change.
    field: Option<field::Field>,
    /// The outgoing picture, its own bound, and its own field, for as long as
    /// the dissolve runs.
    from: Option<(iced_image::Handle, f32, Option<field::Field>)>,
    /// The incoming picture's opacity, `[0, 1]`. `1.0` at rest.
    t: f32,
}

impl Work {
    /// A record with nothing decoded: the gradient, and no bound at all — a
    /// gradient has no resolution, so *larger than its source* is not a
    /// predicate that applies to it.
    fn bare() -> Self {
        Self {
            handle: None,
            back: None,
            source: f32::INFINITY,
            field: None,
            from: None,
            t: 1.0,
        }
    }

    /// **How far the dissolve may be drawn at `edge`** — `1.0` wherever it may
    /// not be drawn at all.
    ///
    /// The one condition, and it is a refusal rather than a tuning: **both
    /// pictures must be drawn at one edge.** [`art_edge`]'s third term is the
    /// decode's own pixels, so two decodes that do not agree about it are two
    /// different sizes of sleeve — and a dissolve between two sizes is a
    /// dissolve *and* a resize, which is animating geometry and is on
    /// ADR-0020's standing refusal list. Where they disagree the change stays
    /// the hard cut it has always been, and the sleeve is the incoming one at
    /// full strength from the first frame.
    ///
    /// It is asked of the **drawn** edges rather than of the two `source`
    /// values, because the ordinary case is two covers of different native
    /// sizes that the viewport bounds to the same number — a 700 px sleeve and
    /// a 1024 px one are one 634 px square in a 1280 × 860 window, and refusing
    /// those would refuse most of the feature.
    fn dissolve_at(&self, edge: f32, width: f32, height: f32, run: bool) -> f32 {
        let Some((_, from_source, _)) = self.from else {
            return 1.0;
        };
        if (record_edge(width, height, run, from_source) - edge).abs() < 0.5 {
            self.t
        } else {
            1.0
        }
    }
}

/// The work itself at `edge` — the decoded cover over the picture it is
/// replacing, or the wall's own deterministic gradient where a record has none.
///
/// **The dissolve is two layers and one opacity**, which is the cheapest honest
/// crossfade the toolkit offers: the outgoing picture at full strength
/// underneath, the incoming one over it at `t`, so the composite is
/// `old · (1 − t) + new · t` and neither is ever drawn larger than its own
/// source ([`Work::dissolve_at`] is what guarantees the second half). At rest —
/// every frame but the twelve a record change spends — `t` is `1.0` and this is
/// the single `image` it has always been, with no stack and no second layer.
fn sleeve(
    work: &Work,
    t: f32,
    edge: f32,
    rotation: crate::jewel_case::Rotation,
    insert: &crate::jewel_case::Insert,
) -> Element<'static, Message> {
    crate::jewel_case::view(
        edge,
        rotation,
        crate::jewel_case::Art {
            front: work.handle.clone(),
            from: work.from.as_ref().map(|(handle, _, _)| handle.clone()),
            front_opacity: t,
            back: work.back.clone(),
        },
        insert,
    )
}

/// The same work without physical packaging, for the plain-cover visual mode.
fn plain_cover(work: &Work, t: f32, edge: f32, album_id: u64) -> Element<'static, Message> {
    let image = |handle: iced_image::Handle| {
        iced_image(handle)
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
    };
    match (&work.handle, &work.from) {
        (Some(handle), Some((from, _, _))) if t < 1.0 => {
            stack![image(from.clone()), image(handle.clone()).opacity(t)].into()
        }
        (Some(handle), _) => image(handle.clone()).into(),
        (None, _) => crate::views::gradient_block(album_id, edge, 1.0),
    }
}

/// **The field, laid under everything** — the place's z1, and the reason a
/// large screen reads as composed rather than empty.
///
/// A `Space` when the record has no field, which is the room showing through:
/// no art, no hue in the art, or a hero still in flight. Story S7's *"the
/// field falls back to the room, because there is no palette to read"*, and
/// the honest answer rather than a grey wash pretending to be derived from
/// something.
///
/// **It is under, never over.** Nothing is drawn on the sleeve; the field is
/// the room's own colour changed, it dims no artwork, and it is not a scrim —
/// [`crate::field`] carries the argument.
///
/// # One wash, and the three grounds that went
///
/// The owner, 2026-08-10: *"the background fade behind the album art seems to
/// abruptly end beside the track list which looks bad -- the fade should
/// continue under the playlist area too"*.
///
/// This function used to answer a `Ground` — `Ambient` for the record alone,
/// `Split(under_run)` for the two columns, `Still` for the re-stacked one —
/// and in the split case it drew **two washes in a `row!`**. That is worse
/// than the lightness step it was designed as: two gradients side by side do
/// not step at their join, the second **restarts the ramp**, so the join was a
/// hard vertical edge that announced the layout. Doc 12 §5.4's *"the ceiling
/// is lower where type is"* was drawn as *the ceiling is a different object
/// where type is*.
///
/// It is now one wash over the whole body, and the constraint that produced
/// the domains is answered by measurement instead:
/// `field::every_run_row_is_legible_over_the_brightest_field` sweeps every
/// room × every hue × every ink the run column draws against the field's own
/// brightest stop, and every one clears the floor its use implies — the
/// binding case being `paper_faint` at 4.71 : 1 against 4.5.
///
/// **The seam is also why `run_w` no longer has a second consumer here.** The
/// field's domain and the layout's split were two functions of one number
/// that had to be kept in step, and `the_composition_holds_across_the_restack`
/// existed to prove they were. One of them is gone, so they cannot disagree.
///
/// # And the same seam, in time
///
/// The wash **travels with the cover** (ADR-0020's third amendment): `t` here
/// is the *same number* the sleeve's incoming layer is drawn at, so a record
/// change moves the picture and the room together or moves neither. It is one
/// argument rather than two because that is what makes them unable to
/// disagree — a field that cut while the cover dissolved would put the edge the
/// owner just had removed back into the surface, in time instead of in space,
/// and it would be *more* visible there: a wash over the whole body changing in
/// one frame is a light being switched, and the record it belongs to would
/// still be arriving.
fn field_layer(
    from: Option<field::Field>,
    to: Option<field::Field>,
    t: f32,
) -> Element<'static, Message> {
    let room = theme::active();
    let Some(gradient) = field::dissolve(from, to, t, room) else {
        return Space::new().width(Length::Fill).height(Length::Fill).into();
    };
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(gradient.into())),
            ..container::Style::default()
        })
        .into()
}

/// **The whole page, in three layers**: the object, the fade, and everything
/// written on it.
///
/// # Over, not beside
///
/// The marquee hangs from the bottom-left corner and the object stands at the
/// right, and the two **overlap** rather than dividing the width. They divided
/// it for a day, which is how the owner found the fault: *"the album art seems
/// to be quite small when the window is narrower. instead it should just go
/// behind the text surely?"* It should — a reserved column for the title took
/// width off the sleeve at exactly the sizes where the sleeve had least to
/// spare, and the title never wanted that width at the corner it occupies.
///
/// So the object is bound by the stage's **height** alone at every width.
///
/// # One fade, not two
///
/// [`scrim`] is a single layer under all the type and over all the artwork,
/// spanning the body from the top of the object to the bottom of the source
/// band. It began as two — a fade behind the title and the source band's own —
/// and the owner saw the seam where they met: *"it looks like we have two
/// gradients going on… can we instead just have one."* The band draws no
/// ground of its own now; it is the bottom inch of this one.
///
/// The padding belongs to the **tenants**, not to the stack. A scrim inset by
/// the page's margin draws its own edges — a lighter rectangle with two
/// verticals and a horizon, which is worse than the contrast it was put there
/// to fix.
fn stage<'a>(
    object: Element<'a, Message>,
    marquee: Element<'a, Message>,
    source: Option<Source>,
) -> Element<'a, Message> {
    let margins = iced::Padding {
        top: MARGIN,
        right: MARGIN,
        bottom: theme::GAP_XL,
        left: MARGIN,
    };
    let mut written = column![
        Space::new().width(Length::Fill).height(Length::Fill),
        container(marquee).padding(margins),
    ]
    .width(Length::Fill)
    .height(Length::Fill);
    if let Some(source) = source {
        written = written.push(source_link(source));
    }
    iced::widget::stack![
        container(object)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(margins)
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        scrim(),
        written,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// **The fade the title is read against — down to the room's own floor.**
///
/// The marquee is drawn over the artwork, and 64 px of paper-coloured serif
/// over a record sleeve is a contrast gamble taken once per album: some covers
/// are dark and some are a photograph of a sunlit wall. So the page fades to
/// an opaque ground toward its bottom edge, and the title is read against
/// that rather than against whatever the record happened to be.
///
/// # Which ground, and why it is not black
///
/// It **was** black, because the owner asked for black — *"maybe just make it
/// fade to black?"* — while looking at Closing Time, where the room's own
/// floor is very nearly black and the two answers are the same picture. In
/// every light room they are not: Plaster faded a light grey page into a solid
/// black band across its bottom third, and the source band's dark ink went
/// with it into a plane it could not be read on. That is what the owner then
/// read as *"the now playing view still doesn't have the right styling for the
/// text and the gradient area"*, and what *"the colours on the now playing need
/// to be part of the theme"* asked for in the first place.
///
/// [`theme::Palette::wall`] is the answer rather than any other plane because
/// it is what this page **is**: the wall is the ground every place stands on,
/// so the fade ends where the page would have been with no record at all, and
/// the ink written over it is the same [`theme::Palette::paper`] ladder that
/// reads on the wall everywhere else in the product. In a dark room that is
/// still, correctly, almost the black that was asked for.
///
/// Clear across the top three fifths so the sleeve is untouched where nothing
/// is written on it, and solid only at the very bottom — the source band is
/// the last inch of this one fade, and reaching solid early would draw a
/// horizon across the page.
fn scrim() -> Element<'static, Message> {
    let floor = theme::active().wall;
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(
                iced::gradient::Linear::new(std::f32::consts::PI)
                    .add_stop(0.0, iced::Color { a: 0.0, ..floor })
                    .add_stop(0.42, iced::Color { a: 0.0, ..floor })
                    .add_stop(0.72, iced::Color { a: 0.42, ..floor })
                    .add_stop(0.9, iced::Color { a: 0.86, ..floor })
                    .add_stop(1.0, floor)
                    .into(),
            )),
            ..container::Style::default()
        })
        .into()
}

/// A quiet full-width footer at the bottom of Now playing, leading to the
/// source's real page. It is a provenance statement first and a control
/// second: one faint plane, no border, and no navigation chrome beyond the
/// arrow.
fn source_link(source: Source) -> Element<'static, Message> {
    let room = theme::active();
    let message = source.open_message();
    let (kind, name) = match source {
        Source::Playlist { name, .. } => ("Playlist", name),
        Source::Queue { name } => ("Unsaved playlist", name),
        Source::Album { name, .. } => ("Album", name),
    };
    button(
        row![
            column![
                text("Source")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint),
                container(
                    text(name)
                        .size(theme::SIZE_EMPHASIS)
                        .line_height(theme::LEADING_EMPHASIS)
                        .font(theme::MEDIUM)
                        .color(room.paper)
                        .wrapping(text::Wrapping::None),
                )
                .height(Length::Fixed(theme::LINE_EMPHASIS)),
                text(kind)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_muted),
            ]
            .spacing(0)
            .width(Length::Fill),
            text("→")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .color(room.paper_dim),
        ]
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SOURCE_CARD_H))
    // **On the marquee's own margin, not the standard gutter.** The owner:
    // *"the text alignment between the title text and the source bar below
    // looks bad."* It was 40 against the title block's 60, so the two stacked
    // columns of type on this page started twenty pixels apart — close enough
    // to read as a mistake rather than as a second margin. `MARGIN` is the
    // place's own edge and this band is inside the place.
    .padding([theme::GAP_SM, MARGIN])
    // **It rises out of the field rather than sitting on top of it**, and it
    // takes the record's own colour on the way.
    //
    // The owner, twice in a minute: *"make the area where the source is linked
    // a gradient that fades from the top (more transparent) to bottom
    // (solid)"*, and *"make the source area related to the 'colours' of the
    // track."* One change: a vertical wash, clear at the top edge so the field
    // and the sleeve read through it, solid at the bottom where the band meets
    // the window.
    //
    // The tint is [`field::Field::inks`]'s third ink at a low weight over the
    // room's own step — the same well the artist line above draws its colour
    // from, so a record's page is one colour family rather than a coloured
    // line on a grey band. A record with no field falls back to the plain
    // step, which is what this always was.
    // **No ground of its own.** The page's single fade ([`scrim`]) reaches
    // solid black exactly here, so a second gradient starting where that one
    // stopped is what the owner saw as *"two gradients going on… can we
    // instead just have one."* What is left is the press feedback, which a
    // band still owes a pointer that is over it.
    //
    // **The press feedback is a plane, not a white veil.** A 6 % white wash
    // brightens a dark room and does nothing at all to a light one, so the
    // band answered the pointer in six rooms out of sixteen. The page's fade
    // ends on [`theme::Palette::wall`], so one step up from the wall is what a
    // hovered row stands on everywhere else in the product, and it is a
    // *colour the room chose* rather than an amount of white.
    .style(move |_theme, status| {
        let lit = room.step_up(room.wall);
        let ground = match status {
            button::Status::Hovered => Some(lit),
            button::Status::Pressed => Some(room.step_up(lit)),
            button::Status::Active | button::Status::Disabled => None,
        };
        button::Style {
            background: ground.map(iced::Background::Color),
            text_color: room.paper,
            border: iced::Border::default(),
            ..button::Style::default()
        }
    })
    .on_press(message)
    .into()
}

#[cfg(test)]
mod tests {
    /// **Nothing on this page picks its own colour.**
    ///
    /// Twice now the same class of fault has shipped here, and both times it
    /// looked correct in the room it was written in. The scrim faded to
    /// `Color::BLACK` because the owner asked for black while looking at
    /// Closing Time, whose floor *is* nearly black; in Plaster the same line
    /// laid a solid black band across the bottom third of a light page and
    /// took the source band's ink down with it. The band's press wash was
    /// `Color::WHITE` at 6 %, which lights a dark room and is invisible in a
    /// light one. Both read as *"the colours on the now playing need to be
    /// part of the theme otherwise it looks weird"*.
    ///
    /// So the pin is on the class rather than on either instance: this page
    /// draws over artwork, which is the one surface in the product where a
    /// literal can look deliberate, and every colour it draws has to come from
    /// [`theme::Palette`] or from the record's own [`crate::field::Field`].
    /// The exception is a **zero** alpha, where the colour is not drawn at all
    /// and only names the ramp's other end.
    #[test]
    fn every_colour_on_this_page_comes_from_the_room_or_the_record() {
        let source = include_str!("now_playing.rs").replace("\r\n", "\n");
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        for (line_no, line) in shipped.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") || code.starts_with("///") {
                continue;
            }
            for literal in ["Color::BLACK", "Color::WHITE", "Color::from_rgb"] {
                assert!(
                    !code.contains(literal),
                    "now_playing.rs:{}: `{literal}` is a colour this page chose \
                     for itself. Every plane here has to come from the room \
                     (`theme::active()`) or from the record (`Field::inks`), or \
                     it is right in one room and wrong in the other fifteen.",
                    line_no + 1
                );
            }
        }
    }

    /// **The page fades to the room's floor, and the floor is the wall.**
    ///
    /// Stated against [`theme::Palette::wall`] by name rather than against the
    /// pixels, because the point is *which plane* — the wall is the ground
    /// every place in the product stands on, so the fade ends where this page
    /// would have been with no record at all, and the `paper` ladder written
    /// over it is the ladder that reads on the wall everywhere else.
    #[test]
    fn the_scrim_fades_to_the_rooms_own_wall() {
        let source = include_str!("now_playing.rs").replace("\r\n", "\n");
        let rest = source.split_once("fn scrim()").expect("the scrim").1;
        let body = &rest[..rest.find("\n}\n").expect("a function ends")];
        assert!(
            body.contains("theme::active().wall"),
            "the scrim no longer ends on the room's own wall"
        );
        assert_eq!(
            body.matches("add_stop").count(),
            5,
            "the fade's shape changed; if that is intended, restate the ramp \
             here — the top three fifths are clear on purpose and solid \
             arrives only at the very bottom edge"
        );
    }

    /// **The source band's type stands on the marquee's own margin.**
    ///
    /// The owner: *"the text alignment between the title text and the source
    /// bar below looks bad."* It was, and by exactly twenty pixels: the title
    /// block sits at [`MARGIN`] and the band was padded with
    /// [`theme::HANG`], so the page's two stacked columns of type started at
    /// 60 and 40. Close enough to read as a mistake rather than as a second,
    /// deliberate margin.
    ///
    /// Stated as an identity of the *tokens* rather than of two numbers that
    /// happen to agree, so moving `MARGIN` moves both edges together.
    #[test]
    fn the_source_band_shares_the_title_s_left_edge() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/now_playing.rs"),
        )
        .expect("this file");
        let shipped = source.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(
            shipped.contains("padding([theme::GAP_SM, MARGIN])"),
            "the source band is not padded to the place's own margin — its \
             text will not line up with the title above it"
        );
        assert!(
            !shipped.contains("padding([theme::GAP_SM, theme::HANG])"),
            "the source band is back on the standard gutter, twenty pixels \
             inside the title's edge"
        );
    }

    /// **The album takes the room the composition actually leaves it.**
    ///
    /// Told twice that it was too small — *"not tiny on the now playing
    /// screen"*, then *"very small and cramped up into the corner"* — and the
    /// second telling is what says the fault is the method rather than the
    /// constant. Both drafts picked a **share** of something that was not the
    /// object's own region, so both left room on the page that nothing used.
    ///
    /// This asserts against the region instead: whatever else changes, the
    /// object fills what it is given unless its own pixels run out first. A
    /// share test could be satisfied by a number that is still wrong; this one
    /// cannot, because there is nothing left over for it to be wrong by.
    #[test]
    fn the_album_fills_the_room_the_page_leaves_it() {
        for (width, height) in [(1048.0, 710.0), (1688.0, 952.0), (2328.0, 1312.0)] {
            let plenty = f32::INFINITY;
            let edge = marquee_edge(width, height, plenty);
            let (region_w, region_h) = object_region(width, height);
            let room = region_w.min(region_h);
            assert!(
                (edge - room).abs() < 0.5,
                "at {width}x{height} the object is {edge:.0} px in a {room:.0} px \
                 region — {:.0} px of the page is reserved for it and drawing \
                 nothing",
                room - edge
            );
        }
    }

    /// **A narrower window does not make the album smaller.**
    ///
    /// The premise this replaces was the opposite one: the object and the
    /// title divided the width, and this held the title's share from below.
    /// The owner found what that cost — *"the album art seems to be quite
    /// small when the window is narrower. instead it should just go behind the
    /// text surely?"* — and he is right, because the title occupies the
    /// bottom-left corner and never wanted the width it was being given.
    ///
    /// So they overlap, and the object is bound by the stage's **height**
    /// alone. This is that claim: at a fixed height, narrowing the window
    /// leaves the album exactly where it was, until the window is narrow
    /// enough that width is genuinely the smaller of the two.
    #[test]
    fn narrowing_the_window_does_not_shrink_the_album() {
        let height = 952.0;
        let (_, down) = object_region(3000.0, height);
        for width in [1200.0_f32, 1688.0, 2328.0, 3000.0] {
            let edge = marquee_edge(width, height, f32::INFINITY);
            assert!(
                (edge - down).abs() < 0.5,
                "at {width}x{height} the album is {edge:.0} px where the height \
                 alone allows {down:.0} — something is still taking width off it"
            );
        }
        // And the title's measure is the window's again, not what the object
        // left over: it depends on the window and on nothing else.
        assert!(
            marquee_measure(1688.0, height) > marquee_measure(1200.0, height),
            "the title stopped growing with the window"
        );
        // The footer keeps its own band whatever the object does.
        for (width, height) in [(1048.0, 710.0), (1688.0, 952.0), (2328.0, 1312.0)] {
            let (_, region_h) = object_region(width, height);
            assert!(
                height - region_h >= SOURCE_CARD_H,
                "the object's region has eaten the source footer"
            );
        }
    }

    /// **And it is still never larger than its own pixels**, which is the one
    /// part of the old composition that may not move whatever the share is.
    #[test]
    fn a_bigger_object_still_never_upscales_a_cover() {
        for source in [96.0_f32, 300.0, 500.0, 800.0, 1400.0] {
            for (width, height) in [(1048.0, 710.0), (1688.0, 854.0), (3608.0, 2084.0)] {
                let edge = marquee_edge(width, height, source);
                assert!(
                    edge <= source.max(1.0),
                    "a {source} px cover was drawn at {edge} px on a \
                     {width}x{height} stage"
                );
            }
        }
    }

    use super::*;

    #[test]
    fn the_sounding_library_track_carries_the_shared_favourite_action() {
        let source = include_str!("now_playing.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Now Playing source has a non-test head");
        assert!(source.contains("crate::views::page::favourite_slot(path, selected)"));
        assert!(source.contains("favourite: Option<(&'a std::path::Path, bool)>"));
    }

    /// Every window side the sweeps below walk, in the shape `art_edge`'s
    /// original test walks them.
    fn sides() -> impl Iterator<Item = f32> {
        (400..=4000)
            .step_by(7)
            .map(|side| f32::from(u16::try_from(side).expect("a window side fits u16")))
    }

    /// The decoded sizes the sweeps walk: two below [`theme::ART_MIN`], one
    /// between the tiers, the thumbnail's own ceiling, the hero's own ceiling,
    /// and a source large enough that the viewport is always the binding term
    /// (doc 12 §5.2's list, plus 240 and 3000 for the two ends it omits).
    const SOURCES: [f32; 6] = [120.0, 240.0, 320.0, 500.0, 1024.0, 3000.0];

    /// **The marquee stands in the same place whether or not there is an
    /// object** — which is the whole reason the two branches became one
    /// composition. The measure depends on the window and on nothing else: not
    /// on the artwork's size, not on its shape, not on whether it exists.
    #[test]
    fn the_marquee_is_anchored_to_the_place_and_not_to_the_artwork() {
        let height = 900.0;
        for width in [320.0_f32, 760.0, 1280.0, 1920.0, 3840.0] {
            let measure = marquee_measure(width, height);
            assert!(measure > 0.0);
            assert!(
                measure <= (width - 2.0 * MARGIN).max(1.0),
                "the marquee ran past the place's own margins at {width}"
            );
            assert!(
                measure <= theme::LIST_MEASURE * 1.25,
                "the title runs a line no eye tracks at {width}"
            );
        }
        // **Wider windows never give the title less**, and the cap is reached
        // rather than approached.
        //
        // It used to say *more*, which stopped being true when the album
        // started taking the width first: between 760 and 1280 every extra
        // pixel goes to the sleeve, and the title sits at its floor. That is
        // the owner's *"it should be taking up most of the area"* expressed as
        // an ordering — the album grows until it is bound by the stage's
        // height, and only the width left over after that reaches the title.
        let mut previous = 0.0_f32;
        for width in [320.0_f32, 760.0, 1280.0, 1920.0, 2560.0, 3840.0] {
            let measure = marquee_measure(width, height);
            assert!(
                measure >= previous - 0.5,
                "the title got narrower at {width} than at the size below it"
            );
            previous = measure;
        }
        assert!(
            (marquee_measure(2560.0, height) - marquee_measure(3840.0, height)).abs()
                < f32::EPSILON,
            "a 4K window sets a wider line instead of a longer one"
        );
    }

    /// **The ladder steps down and never off.** Three rungs, chosen by length,
    /// and the longest title in the owner's library still lands on one of
    /// them — there is no fourth case where the title is dropped or cut.
    #[test]
    fn the_title_ladder_has_a_rung_for_every_length() {
        let rungs = [
            ("Ochre", theme::SIZE_MARQUEE),
            ("Aren't We All Running?", theme::SIZE_MARQUEE),
            (
                "I Am A Man Of Constant Sorrow (with band)",
                theme::SIZE_DISPLAY,
            ),
            (
                "Menus propos enfantins (Childish Chatter), for piano (from -Enfantines-)- \
                 Chant guerrier du roi des haricots. Mouvt de Marche",
                theme::SIZE_HERO,
            ),
        ];
        for (title, expected) in rungs {
            let (size, leading) = marquee_type(title);
            assert!(
                (size - expected).abs() < f32::EPSILON,
                "{:?} ({} chars) set at {size}, not {expected}",
                &title[..title.len().min(30)],
                title.chars().count()
            );
            // Every rung carries its own leading rather than iced's default.
            assert!(
                leading > 1.0 && leading < 1.4,
                "rung {size} has leading {leading}"
            );
        }
        // The steps only ever go down as the title grows.
        let mut previous = f32::MAX;
        for length in [10_usize, 40, 100, 200] {
            let (size, _) = marquee_type(&"a".repeat(length));
            assert!(
                size <= previous,
                "the ladder went back up at {length} chars"
            );
            previous = size;
        }
    }

    /// **The object is bounded by the room and by its own pixels**, and never
    /// upscaled — the one rule the old composition had that this one keeps.
    ///
    /// The upper bound used to be the flat `LIST_MEASURE / 2` the share was
    /// capped at, asserted as *"the object took the page back"*. That was the
    /// cap being checked against itself, and it is why nothing failed when the
    /// share turned out to be far too small for the page: a ceiling cannot
    /// notice a floor. It is stated against the **stage** now, which is the
    /// thing the object actually has to share.
    #[test]
    fn the_object_never_outgrows_its_source_or_the_room() {
        for width in [320.0_f32, 1280.0, 3840.0] {
            for height in [384.0_f32, 860.0, 2160.0] {
                for source in SOURCES {
                    let edge = marquee_edge(width, height, source);
                    assert!(
                        edge <= source,
                        "{width}x{height}: upscaled a {source} px source"
                    );
                    assert!(edge > 0.0);
                    let (region_w, region_h) = object_region(width, height);
                    let room = region_w.min(region_h);
                    assert!(
                        edge <= room.max(theme::CONTINUE_SLEEVE) + 0.5,
                        "{width}x{height}: the object took the page back — \
                         {edge:.0} px in a {room:.0} px region, so it is \
                         drawing over the marquee or the footer"
                    );
                }
            }
        }
    }

    /// **The kiosk is this surface at a larger size**, and it is a property of
    /// the arithmetic rather than a plan: the work's edge grows with the
    /// viewport, monotonically, and stops **where the source stops** — which
    /// since step A2 is the only ceiling there is.
    #[test]
    fn the_work_grows_with_the_window_and_stops_at_its_source() {
        for source in SOURCES {
            for run in [false, true] {
                let mut previous = 0.0_f32;
                for side in sides() {
                    let edge = record_edge(side, side, run, source);
                    assert!(edge >= theme::ART_MIN.min(source), "{side}: {edge}");
                    assert!(edge <= source, "{side}: {edge} px drawn from {source} px");
                    assert!(
                        edge >= previous,
                        "{side}: the work shrank as the window grew (run {run})"
                    );
                    previous = edge;
                }
            }
        }
        // **A 4 K panel is bound by the file and nothing else.** This is the
        // whole of step A2 in one assertion, and the `NOW_PLAYING_MAX` 720 it
        // replaced is what made it false: a well-kept collection now fills the
        // panel, and a modest one is drawn honestly small on a field rather
        // than upscaled to look large.
        for run in [false, true] {
            for source in SOURCES {
                let edge = record_edge(3840.0, 2160.0, run, source);
                assert!(
                    (edge - source.min(2160.0 - 80.0 - BELOW)).abs() < f32::EPSILON,
                    "run {run}, source {source}: {edge}"
                );
            }
            // The two rows of doc 12 §5.5's table that A2 exists for. Both are
            // `NOW_PLAYING_MAX` 720 in the build before this one.
            assert!((record_edge(3840.0, 2160.0, run, 1024.0) - 1024.0).abs() < f32::EPSILON);
            assert!((record_edge(2560.0, 1440.0, run, 1024.0) - 1024.0).abs() < f32::EPSILON);
        }
    }

    /// **The surface never draws art larger than its source** — doc 12 §5.2's
    /// test, verbatim, and the refusal this product has always stated and has
    /// never been able to make true here (ADR-0029 §Context 2).
    ///
    /// Swept the way the wall's own
    /// `the_wall_never_draws_art_larger_than_its_source` is swept
    /// (`shelf.rs:1509–1530`), because it is the same claim about the other
    /// surface that draws artwork.
    ///
    /// **`theme::ART_MIN` does not exempt a small cover**, and that is the one
    /// place this test disagrees with the formula doc 12 §5.2 prints beside
    /// it: `min(…, hero_px).max(ART_MIN)` draws a 120 px cover at 240 and
    /// fails this. The floor is a design statement about when a work stops
    /// being a subject; the source is a fact about how many pixels exist. The
    /// fact wins, story S7 asks for exactly that, and the field is what makes
    /// the result composed rather than broken.
    #[test]
    fn the_now_playing_surface_never_draws_art_larger_than_its_source() {
        for source in SOURCES {
            for side in sides() {
                for run in [false, true] {
                    let beside = run_w(side, side, run);
                    assert!(
                        art_edge(side, side, beside, source) <= source,
                        "{side}² with {beside} beside: {} px drawn from {source} px",
                        art_edge(side, side, beside, source)
                    );
                    // …and the re-stacked head block below `SPLIT_FLOOR` is
                    // bound by the same fact, which is why the clamp lives in
                    // `record_edge` rather than only in `art_edge`.
                    assert!(record_edge(side, side, run, source) <= source, "{side}");
                }
            }
        }
        // **A record with no art at all has no source**, so the deterministic
        // gradient takes whatever the viewport allows — a gradient has no
        // resolution and *larger than its source* is not a predicate that
        // applies to it (story S7, `crate::field`'s second property).
        let unbounded = art_edge(1920.0, 1080.0, 0.0, f32::INFINITY);
        assert!(unbounded.is_finite() && (unbounded - (1080.0 - 80.0 - BELOW)).abs() < 0.001);
    }

    /// **A wide, short window is bounded by its height** — a now-playing
    /// screen whose placard has been pushed off the bottom is not one.
    #[test]
    fn a_short_window_is_bounded_by_its_height() {
        assert!(
            art_edge(2560.0, 600.0, 0.0, f32::INFINITY)
                < art_edge(2560.0, 1400.0, 0.0, f32::INFINITY),
            "the height has to be in the arithmetic"
        );
        // …and it never collapses below the floor, whatever the window does.
        for height in [0.0, 1.0, 120.0, 300.0] {
            for run in [false, true] {
                let edge = art_edge(1280.0, height, run_w(1280.0, height, run), f32::INFINITY);
                assert!((edge - theme::ART_MIN).abs() < f32::EPSILON, "run {run}");
            }
        }
    }

    /// **The placard reserves what it draws** — no transport it does not draw,
    /// and every gap it does.
    ///
    /// Two corrections live in this one number, and they pull opposite ways:
    ///
    /// - ADR-0029's first step took `TRANSPORT_HIT` **32** out, because the
    ///   duplicated transport widget came off this surface and the height it
    ///   had reserved stayed in the arithmetic.
    /// - Step A2 put **16** back for the six-child placard that stood then.
    ///   The progress line now lives only in the persistent bar, so removing
    ///   that child and its adjacent spacing gives 10 px back honestly.
    ///
    /// **The terms are asserted, not the total**, because the total is what a
    /// future step's own additions will change and the terms are what must not
    /// silently grow a transport — or lose a figure — again.
    #[test]
    fn the_placard_reserves_exactly_what_it_draws() {
        // **The column, summed the way iced lays it out**: three identity
        // children, two `GAP_XS` between them, and one `GAP_XL` off the sleeve.
        // If any of those move, this is what
        // catches the reservation not moving with them.
        const CHILDREN: f32 = theme::LINE_HEADING + theme::LINE_DISPLAY + theme::LINE_BODY;
        // **108 since 2026-08-17**, and the twelve it grew by is the title's:
        // the work's name moved from `LINE_HERO` 32 to `LINE_DISPLAY` 44
        // (*"it really needs to pop"*). The reservation had to move with it or
        // the sleeve would be sized against a placard shorter than the one
        // drawn, and the bottom line would fall off the surface — which is the
        // exact failure this test exists to catch, arriving from the other
        // direction.
        const { assert!(BELOW == 108.0) }
        const { assert!(BELOW == theme::GAP_XL + CHILDREN + 2.0 * theme::GAP_XS) }
        // 1280 × 860 with the returns lane collapsed: 1184 × 779 of body,
        // height-bound, and the sleeve is the height less the gutter and the
        // placard.
        let bare = |w: f32, h: f32| art_edge(w, h, 0.0, f32::INFINITY);
        assert!((bare(1184.0, 779.0) - (779.0 - 80.0 - BELOW)).abs() < f32::EPSILON);
        assert!((bare(1184.0, 779.0) - 591.0).abs() < f32::EPSILON);
    }

    #[test]
    fn an_album_source_does_not_repeat_the_album_but_a_playlist_does() {
        let album = Source::Album {
            id: 1,
            name: "Record".into(),
        };
        let playlist = Source::Playlist {
            id: 2,
            name: "Mix".into(),
        };
        let queue = Source::Queue { name: "Run".into() };

        assert!(!show_album_line(Some("Record"), Some(&album)));
        assert!(show_album_line(Some("Record"), Some(&playlist)));
        assert!(show_album_line(Some("Record"), Some(&queue)));
        assert!(show_album_line(Some("Record"), None));
        assert!(!show_album_line(None, Some(&playlist)));
    }

    /// **The record column fits the space it was given**, at every window this
    /// product draws and in the tighter of the two branches.
    ///
    /// The one that overflowed is the split: `container(record)` there spends
    /// a [`theme::HANG`] of padding at *each* end, so the column has
    /// `body_h − 2·HANG` to live in, and a reservation 16 px short of what it
    /// lays out silently costs the last child. This is that claim as
    /// arithmetic rather than as a frame — the frame is in
    /// `docs/design/impl/artwork-at-size/`, and it is what found it.
    #[test]
    fn the_placard_never_overflows_the_column_it_is_drawn_in() {
        for width in sides() {
            for height in sides() {
                for source in SOURCES {
                    let edge = art_edge(width, height, run_w(width, height, true), source);
                    // The split branch's own budget, which is the tight one.
                    let budget = height - 2.0 * theme::HANG;
                    assert!(
                        edge + BELOW <= budget.max(theme::ART_MIN + BELOW),
                        "{width}×{height} source {source}: the work {edge} plus the \
                     placard {BELOW} overflows {budget}"
                    );
                }
            }
        }
    }

    /// **The run costs the record nothing wherever the record is
    /// height-bound** (doc 12 §5.5a's table, stated as the property it is
    /// rather than as six rows).
    ///
    /// The run takes width the record structurally cannot use: above the
    /// tightest window this product draws, `below` is short and a 16 : 9 body
    /// is short before it is narrow, so the record's edge is set by the height
    /// and the column beside it changes nothing. Where the record *is*
    /// width-bound the cost is real, and the sweep pins that it is exactly the
    /// width the run took — recorded as a cost paid rather than hidden.
    #[test]
    fn the_run_costs_the_record_nothing_where_it_is_height_bound() {
        for width in sides() {
            for height in sides() {
                let with = art_edge(width, height, run_w(width, height, true), f32::INFINITY);
                let without = art_edge(width, height, 0.0, f32::INFINITY);
                let beside = run_w(width, height, true);
                if beside <= 0.0 {
                    assert!((with - without).abs() < f32::EPSILON, "{width}×{height}");
                    continue;
                }
                let by_height = height - 2.0 * theme::HANG - BELOW;
                if width - 2.0 * theme::HANG - (beside + theme::GAP_XL) >= by_height {
                    assert!(
                        (with - without).abs() < f32::EPSILON,
                        "{width}×{height}: the run cost the record {} px it was not using",
                        without - with
                    );
                }
                assert!(with <= without, "{width}×{height}");
            }
        }
        // The one row of §5.5a's table where the cost is real: 1280 × 860 with
        // the returns lane open is 1000 px of body, the tightest case this
        // product has, and the record is width-bound there. The remedy is
        // already on screen and already keyed — Ctrl+B collapses the lane and
        // the record comes back — which is why this is a cost paid rather than
        // a cost hidden.
        let (body_w, body_h) = (1000.0, 779.0);
        let bare = |w: f32, beside: f32| art_edge(w, body_h, beside, f32::INFINITY);
        assert!(bare(body_w, run_w(body_w, body_h, true)) < bare(body_w, 0.0));
        // …and with the lane collapsed at the same window, it is free again.
        let body_w = 1184.0;
        assert!(
            (bare(body_w, run_w(body_w, body_h, true)) - bare(body_w, 0.0)).abs() < f32::EPSILON
        );
    }

    /// **The two columns re-stack below the split floor**, swept 400–4000 the
    /// way `art_edge`'s own tests are.
    ///
    /// Below [`theme::SPLIT_FLOOR`] the record cannot be the size it deserves
    /// in any case, so the run takes the measure and the record becomes its
    /// head — **one composition degrading, not a second layout**. Above it the
    /// two stand side by side at every size.
    ///
    /// # The run is no longer flat, and the two claims that replaced that one
    ///
    /// This asserted `split == RUN_MEASURE` at every width until step A4. It is
    /// now `RUN_MEASURE · kiosk_scale`, so what is swept instead is the pair of
    /// properties the flat number was a special case of:
    ///
    /// 1. **The floor is exact.** At or below a [`FAR_FIELD_REF`] work the
    ///    scale is `1.0` and the measure is 440 *to the pixel* — which is what
    ///    makes A4 unable to move any window this product has ever been audited
    ///    at (1280 × 860 and 1920 × 1080 both have `by_height` well under 720).
    /// 2. **The record keeps [`theme::ART_MIN`] anyway.** This is the half of
    ///    [`theme::SPLIT_FLOOR`]'s derivation that a growing run could break,
    ///    and the cap in [`run_w`] is what holds it. It is swept over **both**
    ///    axes here rather than width alone, because a run that scales with the
    ///    height cannot be checked at one height — 784 × 4000 is the case that
    ///    would have gone negative.
    #[test]
    fn the_two_columns_restack_below_the_split_floor() {
        for width in sides() {
            for height in sides() {
                let split = run_w(width, height, true);
                assert_eq!(
                    split > 0.0,
                    width >= theme::SPLIT_FLOOR,
                    "{width}: the split floor is the only condition"
                );
                if split > 0.0 {
                    // Never narrower than the desktop measure, and never so
                    // wide that the record loses its floor.
                    assert!(split >= theme::RUN_MEASURE, "{width}×{height}: {split}");
                    assert!(
                        width - 2.0 * theme::HANG - (split + theme::GAP_XL) >= theme::ART_MIN,
                        "{width}×{height}: the record fell below ART_MIN inside the split"
                    );
                    // …and it is *exactly* the desktop measure wherever the
                    // work this surface can show is one the desktop
                    // composition was designed for.
                    if height - 2.0 * theme::HANG - BELOW <= FAR_FIELD_REF {
                        assert!(
                            (split - theme::RUN_MEASURE).abs() < f32::EPSILON,
                            "{width}×{height}: A4 moved a window it must not"
                        );
                    }
                }
                // The word turned off is the whole body, at every width.
                assert!(
                    (run_w(width, height, false)).abs() < f32::EPSILON,
                    "{width}"
                );
            }
        }
        // The floor bites at a 1064 px window with the lane open and an 880 px
        // window with it collapsed — both below the 1280 the composition
        // audits are taken at, so the regime is real rather than theoretical.
        assert!((run_w(theme::SPLIT_FLOOR - 1.0, 999.0, true)).abs() < f32::EPSILON);
        assert!(run_w(theme::SPLIT_FLOOR, 999.0, true) > 0.0);
        // **At the floor itself the cap is exactly `RUN_MEASURE`**, whatever
        // the height — which is why the record does not lurch across it: the
        // 240 px it gets in the split is the 240 px the head block gives it
        // one pixel below.
        for height in sides() {
            assert!(
                (run_w(theme::SPLIT_FLOOR, height, true) - theme::RUN_MEASURE).abs() < f32::EPSILON,
                "{height}: the cap at the floor is not the measure"
            );
        }
    }

    /// **Doc 12 §5.5a's own table, at the three sizes the frames are taken
    /// at** — the arithmetic step A4 exists to produce, and the measurement in
    /// `docs/design/impl/one-list-drawn-once/` in one assertion.
    ///
    /// Body dimensions, not window: the shell hands [`view`] `body_width` and
    /// `body_height`, and a 2560 × 1440 window with the returns lane standing
    /// is a 2280 × 1359 body.
    ///
    /// The gap the owner reported is the last column: the field between the
    /// work's right edge and the run's left. It was `record_w − edge` with the
    /// record hung left; it is one [`theme::GAP_XL`] now, at every size, because
    /// the pair centres.
    ///
    /// # 1920 moves, and doc 12 §11.2 says it must not
    ///
    /// §11.2: *"the clamp's floor of 1.0 is what keeps every window at or below
    /// 720 px of work pixel-identical to what ships today"*. **A 1920 × 1080
    /// body's work is 823 px, not 720**, so the scale is 1.143 there and the run
    /// goes 440 → 503. The document is not wrong about its own arithmetic; it is
    /// out of date about one input, and this is the same correction §5.5a's
    /// table already carries twice: `below` was 190 when §11.2 was written, it
    /// is [`BELOW`] **96** until steps A5 and A9 build the meter and the feed,
    /// and 44 px of `below` is what puts a 1920 work over the reference.
    ///
    /// **It is allowed to move because nothing a listener can see moves badly.**
    /// The work at 1920 is height-bound at 811 px with the run at 440 and still
    /// 811 px with it at 496 — the run takes width the record structurally
    /// cannot use, which is the property `the_run_costs_the_record_nothing…`
    /// sweeps. What changes is that 32 px of the 323 px hole at that size closes
    /// by measure and the rest closes by centring. Recorded rather than tuned
    /// away: keying the reference to make 1920 land on exactly 1.0 would be a
    /// constant chosen to flatter a table, and the table is the thing that is
    /// stale.
    #[test]
    fn the_run_grows_with_the_panel_and_the_gap_does_not() {
        // (body, source, the run's measure)
        for (body_w, body_h, source, run) in [
            // 1280 × 860, lane open — a 603 px work, under the reference, so
            // the scale's floor holds it at exactly the desktop measure.
            (1000.0_f32, 779.0_f32, 1024.0_f32, 440.0_f32),
            // 1920 × 1080, lane open — an 811 px work, just over. See above.
            // (823 until the display title took 12 px more of the column.)
            (1640.0, 999.0, 1024.0, 440.0 * (811.0 / 720.0)),
            // 2560 × 1440, lane open — the window the owner was looking at.
            (2280.0, 1359.0, 1024.0, 440.0 * (1171.0 / 720.0)),
            // 3840 × 2160, lane collapsed — the scale at its ceiling.
            (3744.0, 2079.0, 3000.0, 440.0 * KIOSK_SCALE_MAX),
        ] {
            let split = run_w(body_w, body_h, true);
            assert!(
                (split - run).abs() < 0.5,
                "{body_w}×{body_h}: the run is {split}, not {run}"
            );
            // The pair fits the body with a `HANG` to spare on each side, which
            // is what makes the centring in `view` safe at every one of them.
            let edge = record_edge(body_w, body_h, true, source);
            let pair = edge + theme::GAP_XL + split;
            assert!(
                pair <= body_w - 2.0 * theme::HANG,
                "{body_w}×{body_h}: the pair {pair} overflows the body"
            );
        }
        // **The defect itself, as arithmetic.** Before A4 and the centring the
        // record's column was the whole of what the run left and the work hung
        // at its left edge, so the bare field between the two was *everything
        // the work could not use* — which grows with the panel and shrinks with
        // the cover, and is therefore worst exactly where the owner met it.
        //
        // Both terms are stated because the gap is a function of two things and
        // the frames and the queue quote disagree about one of them: doc 12
        // §5.5a's note says *"~700 px"* from a **1024 px** cover, and
        // `measure.py` reads **1171** off the real frames because the fixture's
        // covers are **600 px**. Neither is wrong; a smaller cover leaves more
        // field. After the change the gap is one `GAP_XL` at every size and at
        // every cover, which is why there is one figure below and not a table.
        let (body_w, body_h) = (2280.0, 1359.0);
        let hung_left =
            |source: f32| body_w - 2.0 * theme::HANG - theme::RUN_MEASURE - theme::GAP_XL - source;
        assert!((hung_left(1024.0) - 712.0).abs() < 1.0, "doc 12's cover");
        assert!(
            (hung_left(600.0) - 1136.0).abs() < 1.0,
            "the fixture's cover"
        );
        // …and the work at that window is bound by the file rather than by
        // either column, before and after — so none of that field was the run's
        // to give back, and widening the run alone could never have closed it.
        for source in [600.0_f32, 1024.0] {
            let edge = record_edge(body_w, body_h, true, source);
            assert!(
                (edge - source).abs() < f32::EPSILON,
                "source-bound at {edge}"
            );
        }
    }

    /// **The composition holds across the restack** — the layout and the
    /// artwork turn at [`theme::SPLIT_FLOOR`] and nowhere else.
    ///
    /// # The seam this was written for is now unreachable, and that is the news
    ///
    /// It swept a third claim until 2026-08-10: that the **field's domain**
    /// turned at the same width the columns did. A field whose domain changed
    /// at one width while the columns re-stacked at another would have put
    /// ambient light under a scrolling list, or a wall-clamped column under the
    /// work, for the band between the two numbers.
    ///
    /// The owner removed the domains — *"the fade should continue under the
    /// playlist area too"* — so there is one wash over the whole body and
    /// **two numbers that had to agree have become one number**. The strongest
    /// version of a test that two things stay in step is the one you delete
    /// because there is only one thing. What is asserted instead is the
    /// property that outlived it: the record's column always has room to be a
    /// record column inside the split.
    #[test]
    fn the_composition_holds_across_the_restack() {
        for width in sides() {
            // **One floor, two consequences** — the run's column and the
            // record's composition. The field is no longer a third, because it
            // no longer has a width in it.
            //
            // Swept at a kiosk height as well as a desktop one since A4: the
            // run's width is a function of both axes now, and the tall case is
            // the one where its cap does the work.
            for height in [999.0_f32, 2079.0] {
                let beside = run_w(width, height, true);
                assert_eq!(
                    beside > 0.0,
                    width >= theme::SPLIT_FLOOR,
                    "{width}: the layout turned somewhere the floor did not"
                );
                if beside > 0.0 {
                    let record_right = width - theme::HANG - beside - theme::GAP_XL;
                    assert!(
                        record_right >= theme::HANG + theme::ART_MIN,
                        "{width}×{height}: the record column fell under its own \
                         floor inside the split"
                    );
                }
            }
        }

        // **The record does not lurch at the floor.** At the widest window
        // below it the head block is `ART_MIN`; at the narrowest above it the
        // record column is at least that, so the work grows across the seam
        // rather than jumping either way — with a source bound in play and
        // without one.
        for source in SOURCES {
            let below = record_edge(theme::SPLIT_FLOOR - 1.0, 1080.0, true, source);
            let above = record_edge(theme::SPLIT_FLOOR, 1080.0, true, source);
            assert!(
                above >= below,
                "source {source}: {below} → {above} across the floor"
            );
            assert!(below <= source && above <= source, "source {source}");
        }
    }

    /// Now playing is one current-song surface, with provenance as its only
    /// road onward. The queue renderer must not creep back into its body.
    #[test]
    fn the_current_song_links_to_its_source_and_draws_no_queue() {
        let place = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/now_playing.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        // Inspect production code only: the guard's own needles necessarily
        // contain the words it checks have not returned.
        let place = place.split_once("#[cfg(test)]").expect("test boundary").0;
        let app = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("the shell source");
        let bar = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/bottom_bar.rs"),
        )
        .expect("the persistent bar source");
        for token in [
            "Source::Playlist",
            "Source::Queue",
            "Source::Album",
            "Message::OpenPlaylist(*id)",
            "Message::ShowQueue",
            "Message::OpenAlbum(*id)",
        ] {
            assert!(place.contains(token), "the source road lost `{token}`");
        }
        assert!(app.contains("fn now_playing_source(&self)"));
        assert!(
            app.contains("crate::player::RunOrigin::Assembled")
                && app.contains("views::now_playing::Source::Queue"),
            "an unsaved run stopped leading to its editable queue"
        );
        assert!(
            bar.contains("source: Option<Message>") && bar.contains(".on_press(source)"),
            "the persistent track block stopped sharing the source road"
        );
        assert!(
            !place.contains("queue::run_column(")
                && !place.contains("\"Show queue\"")
                && !place.contains("ToggleNowPlayingMode"),
            "the queue or its old mode switch came back"
        );
        assert!(
            !place.contains("views::home::needle"),
            "Now playing duplicated the persistent bar's progress line"
        );
    }

    /// **The run is virtual at kiosk scale** (doc 12 §12 M1's gate).
    ///
    /// `Play all` can reify a whole library into this run, so the column the
    /// merged surface draws must cost the frame what a twelve-track record
    /// does — at 3840 × 2160 as much as at 1280 × 860. The arithmetic is
    /// [`crate::queue_window`]'s and this asserts the surface's own inputs
    /// reach it: a five-figure run, at the kiosk's viewport, builds a bounded
    /// slice.
    #[test]
    fn the_run_is_virtual_at_kiosk_scale() {
        use crate::queue_window::{self, RowShape};
        let rows: Vec<RowShape> = (0..40_000)
            .map(|index| RowShape {
                head: (index % 12 == 0 && index > 0).then_some(true),
                two_line: index % 3 == 0,
            })
            .collect();
        for viewport in [779.0_f32, 999.0, 2079.0] {
            let span = viewport + 2.0 * queue_window::MARGIN;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a positive row count in the tens; the assert below pins it"
            )]
            let bound = (span / queue_window::row_pitch(false)).ceil() as usize + 2;
            assert!(bound < 200, "the bound stays small: {bound}");
            for scroll in [0.0, 12_345.0, 987_654.0] {
                let win = queue_window::window(&rows, scroll, viewport);
                assert!(
                    win.end - win.first <= bound,
                    "{} rows at {viewport} px / offset {scroll} — the run is not virtual",
                    win.end - win.first
                );
            }
        }
    }

    /// A [`Work`] carrying two decodes: an incoming cover bounded at
    /// `to` px and an outgoing one bounded at `from`.
    fn crossing(from: f32, to: f32) -> Work {
        let handle = || iced_image::Handle::from_rgba(1, 1, vec![0_u8; 4]);
        Work {
            handle: Some(handle()),
            back: None,
            source: to,
            field: None,
            from: Some((handle(), from, None)),
            t: 0.5,
        }
    }

    /// **A dissolve is drawn only where both pictures are drawn at one edge**
    /// ([`Work::dissolve_at`]).
    ///
    /// Two claims, and the second is what stops the refusal from eating the
    /// feature:
    ///
    /// 1. Two decodes the viewport bounds to the **same** square dissolve, even
    ///    when their native sizes differ by hundreds of pixels — which is the
    ///    ordinary case, since most covers exceed what a window can show.
    /// 2. Two decodes that resolve to **different** squares do not, at any
    ///    window, in either composition. A dissolve between two sizes would be
    ///    a dissolve *and* a resize, and animating geometry is on ADR-0020's
    ///    standing refusal list.
    #[test]
    fn a_dissolve_is_refused_where_the_two_covers_are_not_one_square() {
        for side in sides() {
            for run in [false, true] {
                // Both above whatever this viewport allows: one square, so the
                // pictures cross.
                let large = crossing(f32::INFINITY, f32::INFINITY);
                let edge = record_edge(side, side, run, large.source);
                assert!(
                    (large.dissolve_at(edge, side, side, run) - 0.5).abs() < f32::EPSILON,
                    "{side} px, run {run}: two unbounded covers are one square and must cross"
                );

                // A cover smaller than the square the other one takes: two
                // sizes, so the change stays the cut it has always been.
                let small = theme::ART_MIN;
                let mixed = crossing(small, f32::INFINITY);
                let mixed_edge = record_edge(side, side, run, mixed.source);
                let expected = if (record_edge(side, side, run, small) - mixed_edge).abs() < 0.5 {
                    0.5
                } else {
                    1.0
                };
                assert!(
                    (mixed.dissolve_at(mixed_edge, side, side, run) - expected).abs()
                        < f32::EPSILON,
                    "{side} px, run {run}: a {small} px cover and an unbounded one"
                );
            }
        }
        // …and at a window where the two genuinely differ, the refusal is the
        // answer rather than an accident of the sweep's step.
        let (w, h) = (1280.0, 860.0);
        let mixed = crossing(240.0, f32::INFINITY);
        let edge = record_edge(w, h, false, mixed.source);
        assert!(
            edge > 240.0,
            "the viewport allows more than the small cover"
        );
        assert!(
            (mixed.dissolve_at(edge, w, h, false) - 1.0).abs() < f32::EPSILON,
            "a 240 px cover and a {edge} px square are not one dissolve"
        );
    }

    /// **A settled surface draws exactly one picture** — the property that
    /// keeps the ordinary frame as cheap as it was before the crossfade
    /// existed.
    ///
    /// `t` at rest is `1.0`, and at `1.0` [`sleeve`] takes the branch with no
    /// `stack!` and no second `image` in it. Asserted through `dissolve_at`
    /// rather than by reading the widget tree, because the number is the thing
    /// the branch turns on.
    #[test]
    fn a_settled_surface_has_nothing_to_dissolve() {
        let bare = Work::bare();
        assert!((bare.t - 1.0).abs() < f32::EPSILON);
        assert!(bare.from.is_none());
        for side in sides() {
            let edge = record_edge(side, side, false, bare.source);
            assert!((bare.dissolve_at(edge, side, side, false) - 1.0).abs() < f32::EPSILON);
        }
    }
}
