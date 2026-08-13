//! **The blocked-library screen** — what baz draws when the library is there
//! and this build will not open it (ADR-0041).
//!
//! # Why it exists
//!
//! The owner, 2026-08-10, running a two-day-old binary against his current
//! library: *"it shows me 'where's your music' which has no browse function
//! and it also tells me the schema version is version 8 if I pick any
//! directory"*. baz had done the right thing underneath — a database from a
//! newer build is refused rather than migrated, which is what protects the
//! newer install — and had then reported it by drawing the **first-run
//! screen**. To a listener whose collection is exactly where they left it,
//! that is the application saying *your library does not exist*: the most
//! alarming thing baz can say, said in the one case where nothing is wrong
//! with their data at all.
//!
//! # The shape: a statement, not a question
//!
//! [`crate::views::setup`] asks *where's your music?* and offers three doors
//! to answer with. Every one of them leads back into the same refusal — which
//! is why he saw the version number *"if I pick any directory"*. So this
//! screen does not ask anything. It says three things, in this order, and the
//! order is the design:
//!
//! 1. **What happened**, in one line at the hero size.
//! 2. **What is safe** — *your music and your playlists are untouched* — which
//!    is the sentence that stops somebody panicking, and which is true rather
//!    than soothing: the database is a derived index of files on disk, the
//!    playlists are `.m3u8` files baz only ever reads at start, and the
//!    refusal happens before a byte is written (`baz_core::index`).
//! 3. **What to do**, which for the newer-baz case is *put the newer baz
//!    back* and for the rest is a control on this screen.
//!
//! # One surface, three reasons
//!
//! [`crate::app::Blockage`] has three variants and this file has one layout.
//! The alternative — a screen each for the downgrade, the corrupt file and
//! the machine with no data directory — would be three surfaces to keep in
//! agreement about a sentence they all have to say identically. What differs
//! between them is words and *which controls exist*, and both are data here.
//!
//! # The controls, and the rule they obey
//!
//! **Absent, not disabled** (ADR-0028's rule, kept): `Try again` appears only
//! where trying again could give a different answer, and the set-aside only
//! where there is a file to move.
//!
//! The set-aside is the only thing in baz that touches a database this build
//! has refused to read, and it is fenced three ways:
//!
//! - it is **never the primary control** and never the only one;
//! - the first press **reveals and does not act** — it shows a paragraph
//!   naming exactly what a new index costs, and a second word that acts;
//! - it **renames**, so nothing is discarded: `baz_core::index::set_aside`
//!   moves `library.db` (and its write-ahead log) to `library.db.set-aside-1`
//!   and renaming it back restores it exactly, which is asserted rather than
//!   claimed.
//!
//! # No app bar, and why
//!
//! Every *place* wears the app bar (ADR-0040). This is not a place and it does
//! not, exactly as the first-run screen does not: the bar's zone 3 is the
//! display options, which need a wall of records, and its zone 4 is the door
//! to Settings, which is a place inside a library that has not opened. A bar
//! with two dead zones is a worse statement than no bar, and the window keeps
//! the system's own decorations to be closed by. What the screen does carry is
//! the wordmark, unlit, so it still reads as baz and not as a crash dialogue.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length, alignment};

use crate::app::{Blockage, Blocked, Message};
use crate::theme;

/// The width of the block, in logical px — set by **reading comfort**, not by
/// a control.
///
/// The first-run screen's block is 360 because it is built around a 360 px
/// field and the audit's defect 12 was 93 px of empty outline sticking out
/// past the copy. There is no field here and the copy is three or four
/// sentences rather than one, so the measure is the measure: at the body size
/// this holds roughly 60 characters a line, inside the 45–75 a paragraph is
/// read fastest at.
const MEASURE: f32 = 460.0;

/// How much of the window's slack sits above the block, against [`BELOW`] —
/// the first-run screen's own proportion, for its own reason: a single block
/// on an empty wall sits above the middle or it reads as having sunk.
const ABOVE: u16 = 2;
/// The portion of the window's slack below the block. See [`ABOVE`].
const BELOW: u16 = 3;

/// **What the screen says**, resolved from the blockage before any of it is
/// laid out.
///
/// The words are separated from the composition on purpose: the three reasons
/// differ in exactly these four strings and in which controls exist, and
/// keeping them together makes that comparable — the sentence that must be
/// identical in all three (`safe`) is visibly identical, and the ones that
/// must differ visibly differ.
struct Words {
    /// One line, at the hero size: what happened.
    what: String,
    /// What is safe, and why it is safe. **Every reason says this**, because
    /// in every one of them it is true.
    safe: String,
    /// The machine's own detail — the version pair, or the underlying error.
    /// Set apart in faint ink: it is for a bug report, not for the decision.
    fact: String,
    /// What to do about it, in the listener's terms.
    next: String,
}

impl Words {
    fn of(why: &Blockage) -> Self {
        match why {
            Blockage::NewerBaz { found } => Self {
                what: "This library was made by a newer baz.".to_owned(),
                safe: "Your music and your playlists are untouched. baz read the \
                       version stamp on the library file and stopped there — it has \
                       not opened the library, changed it, or moved a file of yours."
                    .to_owned(),
                fact: format!(
                    "The library is version {found} · this baz reads version {}",
                    baz_core::index::SCHEMA_VERSION
                ),
                next: "Run the newer baz again and your library opens exactly as it \
                       did. Nothing needs repairing first."
                    .to_owned(),
            },
            Blockage::Unreadable { detail } => Self {
                what: "baz could not open its library.".to_owned(),
                safe: "Your music and your playlists are untouched. The library is \
                       only an index baz builds by reading your folders — your files, \
                       your tags and your .m3u8 playlists are not in it and are not \
                       affected."
                    .to_owned(),
                fact: detail.clone(),
                next: "If the file is on a disk or a share that is not ready yet, \
                       Try again once it is. Otherwise the index can be built afresh."
                    .to_owned(),
            },
            Blockage::Nowhere { detail } => Self {
                what: "baz has nowhere to keep its library.".to_owned(),
                safe: "Your music and your playlists are untouched. baz keeps one \
                       small index file in your data folder and could not get to one \
                       on this system."
                    .to_owned(),
                fact: detail.clone(),
                next: "Setting XDG_DATA_HOME to a folder baz can write to, and \
                       starting baz again, is the whole of the fix."
                    .to_owned(),
            },
        }
    }
}

/// The blocked-library screen. See the module docs for the shape and for why
/// it is one screen rather than three.
pub(crate) fn view(blocked: &Blocked) -> Element<'_, Message> {
    let room = theme::active();
    let words = Words::of(&blocked.why);
    let mut content = column![
        column![
            // The wordmark, unlit — the first-run screen's own reasoning. The
            // one accent this room reserves means playback truth, and there is
            // no playback here.
            text("baz")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .color(room.heading()),
            text(words.what)
                .size(theme::SIZE_HERO)
                .line_height(theme::LEADING_HERO)
                .font(theme::SEMIBOLD)
                .color(room.paper),
        ]
        .spacing(theme::GAP_SM),
        // **The sentence that stops somebody panicking**, at body size and in
        // near-full ink rather than as a footnote: it is the most important
        // thing on the screen and it is placed where the eye goes after the
        // headline.
        text(words.safe)
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .color(room.paper_dim),
        text(words.next)
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .color(room.paper_dim),
    ]
    .spacing(theme::GAP_LG)
    .align_x(iced::Alignment::Start);

    // The machine's own words, and where the file is — set apart, quiet, and
    // together, because they are the two things somebody filing a report or
    // opening a file manager needs and neither is part of the decision.
    let mut readout = column![
        text(words.fact)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_XXS);
    if let Some(path) = &blocked.db_path {
        readout = readout.push(
            text(path.display().to_string())
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION)
                .color(room.heading()),
        );
    }
    content = content.push(readout);

    if let Some(trouble) = &blocked.trouble {
        content = content.push(
            text(trouble.as_str())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }

    if let Some(controls) = controls(blocked) {
        content = content.push(controls);
    }

    container(
        column![
            Space::new().height(Length::FillPortion(ABOVE)),
            content.width(Length::Fixed(MEASURE)),
            Space::new().height(Length::FillPortion(BELOW)),
        ]
        .align_x(iced::Alignment::Center),
    )
    .center_x(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// **Which words this screen is offering right now**, as data.
///
/// The whole of the two-step safeguard is in this function's shape, which is
/// why it is a list of pairs and not a tree of widgets: while
/// [`Blocked::setting_aside`] is down, [`Message::LibrarySetAside`] is **not
/// in the returned list at all**, so it is not merely hidden or disabled — no
/// press on the screen can produce it. That is a property a test can hold, and
/// `the_word_that_moves_the_database_is_unreachable_until_the_cost_is_shown`
/// holds it.
fn acts(blocked: &Blocked) -> Vec<(&'static str, Message)> {
    if blocked.setting_aside {
        return vec![
            ("Set aside and start over", Message::LibrarySetAside),
            ("Keep it", Message::LibrarySetAsideAsked(false)),
        ];
    }
    let mut words = Vec::new();
    if blocked.can_retry() {
        words.push(("Try again", Message::LibraryRetry));
    }
    if blocked.can_set_aside() {
        words.push((
            "Set this library aside\u{2026}",
            Message::LibrarySetAsideAsked(true),
        ));
    }
    words
}

/// **What starting a new index costs**, in the listener's terms — the
/// paragraph the first press reveals, above the word that spends it.
///
/// It differs by reason in its first clause and only there, because the
/// disposition differs: for a corrupt index a new one **is** the repair, and
/// for a downgrade it is the wrong move, offered only so that a listener who
/// cannot get the newer baz back is not left with an application that will not
/// start. Saying so is not hedging; a screen that offered the two identically
/// would be recommending the wrong one half the time.
fn cost(why: &Blockage) -> &'static str {
    if matches!(why, Blockage::NewerBaz { .. }) {
        "This is not the fix — the newer baz opens this library as it is. \
         Setting it aside renames the file and starts an empty index over the \
         same folders: nothing is deleted, and renaming it back restores it \
         exactly. What a new index costs is the ADDED dates — every record \
         files under today until you go back."
    } else {
        "Setting it aside renames the file and starts an empty index over the \
         same folders. Nothing is deleted, and renaming it back restores it \
         exactly. What a new index costs is the ADDED dates — every record \
         files under today."
    }
}

/// The screen's acts, laid out, or `None` where it has none to offer.
///
/// The Settings place's *Remove → Forget / Keep* is the pattern: the cost is
/// stated in full ink immediately above the two words that answer it, so the
/// sentence and the press cannot be separated by a scroll or a glance.
fn controls(blocked: &Blocked) -> Option<Element<'_, Message>> {
    let room = theme::active();
    let acts = acts(blocked);
    if acts.is_empty() {
        return None;
    }
    let mut words = row![].spacing(theme::GAP_XXS);
    for (label, message) in acts {
        words = words.push(word(label, message));
    }
    if !blocked.setting_aside {
        return Some(words.into());
    }
    Some(
        column![
            text(cost(&blocked.why))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper),
            words,
        ]
        .spacing(theme::GAP_SM)
        .into(),
    )
}

/// One word-control, in the anatomy `Browse…` established on the first-run
/// screen (ADR-0025 §1): a quiet word at the product's one control height,
/// no fill and no border.
///
/// **No `theme::primary` on this screen, deliberately.** That style is the
/// lamp outline, and the lamp is reserved for playback truth — an amber
/// control on the one surface where nothing can play would spend the reserved
/// signal on an apology.
///
/// The horizontal padding is the **group-key row's** `GAP_XS` and not
/// `Browse…`'s `GAP_SM`, for the group-key row's own stated reason applied to
/// a stricter case: these words are the last line of a left-aligned column of
/// type, and every line of that column starts on one pixel. Eight points of
/// inset put the first word visibly right of the paragraph above it — the
/// composition audit's defect 12, one line down. Four is enough that the hover
/// wash is not tight against the glyphs and small enough that the row still
/// reads as a line of type rather than as boxes.
fn word(label: &'static str, message: Message) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_XS))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(message)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// **Every reason says the sentence.** It is the one line on this screen
    /// whose absence would be a defect rather than a difference — a listener
    /// meeting a corrupt index needs it exactly as much as one meeting a
    /// downgrade, and it is true in all three cases for the same reason.
    #[test]
    fn all_three_reasons_say_the_music_and_the_playlists_are_untouched() {
        for why in [
            Blockage::NewerBaz { found: 99 },
            Blockage::Unreadable {
                detail: "disk image is malformed".to_owned(),
            },
            Blockage::Nowhere {
                detail: "no data directory".to_owned(),
            },
        ] {
            let words = Words::of(&why);
            assert!(
                words
                    .safe
                    .contains("Your music and your playlists are untouched"),
                "a reason that does not say what is safe: {}",
                words.what
            );
            // …and none of them asks the question the first-run screen asks.
            assert!(
                !words.what.contains('?'),
                "a blocked screen that asks a question: {}",
                words.what
            );
        }
    }

    /// **The newer-baz statement names both versions**, which is the whole of
    /// what the listener needs to know which build to go back to. The owner's
    /// report was that the old screen told him *"the schema version is version
    /// 8"* — one number, the wrong one, and no explanation of it.
    #[test]
    fn the_newer_baz_statement_names_both_versions() {
        let words = Words::of(&Blockage::NewerBaz { found: 12 });
        assert!(words.fact.contains("version 12"), "{}", words.fact);
        assert!(
            words
                .fact
                .contains(&format!("version {}", baz_core::index::SCHEMA_VERSION)),
            "{}",
            words.fact
        );
        assert!(
            words.next.contains("newer baz"),
            "the statement must say what actually fixes it: {}",
            words.next
        );
    }

    /// **Retrying is offered only where it could change the answer.** A schema
    /// version is the same number on the second read, and a control that is
    /// certain to fail is a control that teaches the listener baz's words
    /// cannot be trusted.
    #[test]
    fn retry_is_absent_on_the_deterministic_refusal_and_present_otherwise() {
        let newer = Blocked::new(Blockage::NewerBaz { found: 9 }, None, Vec::new());
        assert!(!newer.can_retry());
        let unreadable = Blocked::new(
            Blockage::Unreadable {
                detail: "locked".to_owned(),
            },
            None,
            Vec::new(),
        );
        assert!(unreadable.can_retry());
    }

    /// **The set-aside needs a file to move**, and is absent without one — a
    /// `Nowhere` has no database, and a path that does not exist has nothing
    /// behind the word.
    #[test]
    fn setting_aside_is_absent_when_there_is_nothing_to_set_aside() {
        let nowhere = Blocked::new(
            Blockage::Nowhere {
                detail: "no data directory".to_owned(),
            },
            Some(PathBuf::from("/nonexistent/baz/library.db")),
            Vec::new(),
        );
        assert!(!nowhere.can_set_aside());
        let missing = Blocked::new(
            Blockage::NewerBaz { found: 9 },
            Some(PathBuf::from("/nonexistent/baz/library.db")),
            Vec::new(),
        );
        assert!(!missing.can_set_aside());

        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("library.db");
        std::fs::write(&db, b"x").expect("write");
        let real = Blocked::new(Blockage::NewerBaz { found: 9 }, Some(db), Vec::new());
        assert!(real.can_set_aside());
    }

    /// **The first press reveals; it never acts.**
    ///
    /// The strong form of the claim: before the reveal,
    /// [`Message::LibrarySetAside`] is not among the messages this screen can
    /// emit *at all* — not hidden, not disabled, not reachable. After it, the
    /// word that acts stands beside a way out, and it is not the only word.
    #[test]
    fn the_word_that_moves_the_database_is_unreachable_until_the_cost_is_shown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("library.db");
        std::fs::write(&db, b"x").expect("write");
        let mut blocked = Blocked::new(Blockage::NewerBaz { found: 9 }, Some(db), Vec::new());

        let before = acts(&blocked);
        assert!(!before.is_empty(), "the screen offered nothing at all");
        assert!(
            !before
                .iter()
                .any(|(_, m)| matches!(m, Message::LibrarySetAside)),
            "one press would have moved the database"
        );

        blocked.setting_aside = true;
        let after = acts(&blocked);
        assert!(
            after
                .iter()
                .any(|(_, m)| matches!(m, Message::LibrarySetAside)),
            "the confirmed press is not reachable after the cost is shown"
        );
        assert!(
            after
                .iter()
                .any(|(_, m)| matches!(m, Message::LibrarySetAsideAsked(false))),
            "a two-step with no way back is a one-step with an extra press"
        );
        assert!(
            !matches!(after.first(), Some((_, Message::LibrarySetAsideAsked(_)))),
            "fixture drifted: the acting word should lead the pair"
        );
    }

    /// The revealed paragraph **names what is lost** and, on a downgrade, says
    /// plainly that it is not the repair. Both sentences are the licence for
    /// offering the control at all.
    #[test]
    fn the_cost_names_the_added_dates_and_refuses_to_pose_as_the_fix() {
        let downgrade = cost(&Blockage::NewerBaz { found: 9 });
        assert!(downgrade.contains("ADDED dates"), "{downgrade}");
        assert!(downgrade.contains("othing is deleted"), "{downgrade}");
        assert!(downgrade.contains("not the fix"), "{downgrade}");

        let corrupt = cost(&Blockage::Unreadable {
            detail: "malformed".to_owned(),
        });
        assert!(corrupt.contains("ADDED dates"), "{corrupt}");
        assert!(corrupt.contains("othing is deleted"), "{corrupt}");
        assert!(
            !corrupt.contains("not the fix"),
            "a new index *is* the repair for an unreadable one: {corrupt}"
        );
    }
}
