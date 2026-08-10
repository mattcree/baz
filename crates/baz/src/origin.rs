//! **The list a run was reified from** — origin, never a live link.
//!
//! ADR-0006 layer 1: pure, iced-free, unit-tested. A run in, a name and a key
//! out; no window, no engine, no disk.
//!
//! # The model
//!
//! The owner, 2026-08-10: *"probably the basic model is that every album has a
//! playlist implicitly… and so when we track the state of what is playing now
//! or what our recent plays were… it should be basically which playlist and
//! which track."*
//!
//! So **everything that plays is a list and a cursor**, and what distinguishes
//! one list from another is the identity it carries. ADR-0023 §1 already said
//! that the playing context is *"reified into the queue at the moment of the
//! gesture and then discarded"* — and what is reified is the **contents**,
//! while what was discarded is the **identity**. This type is that identity.
//!
//! # Three properties, each load-bearing (ADR-0034 §1)
//!
//! - **It is inert.** Nothing reads an `Origin` to decide what plays next. It
//!   is read to *say* things and to *file* things. ADR-0023 §1 refuses a
//!   context object that keeps acting, and this is not one.
//! - **The name is stored, not resolved.** A key alone would print nothing
//!   after a rescan renamed the record, and a history's whole job is to be
//!   readable years later. The key is for joining; the name is for reading;
//!   neither is derived from the other at read time.
//! - **A list you can be *in* is not a list you can add *to*.** Only a file is
//!   a destination — [`Origin::is_destination`], and there is exactly one
//!   variant that answers `true`.
//!
//! # This is `implicit::Origin`, grown up
//!
//! `cad9f5a` shipped `implicit::Origin` as a one-variant enum naming the lists
//! with no file behind them. Two enums both called `Origin` — one naming
//! fileless lists, one naming runs' lists — would be the worst possible
//! outcome of two people answering the same sentence of the owner's, so this
//! is the same type with the file-backed kinds added (ADR-0034 §1.4).
//! [`crate::implicit`] re-exports it, and `ImplicitList` keeps its exact
//! meaning, defined as `origin.file().is_none()`.
//!
//! What the promotion spends is `Copy` and `const`: three kinds carry a
//! `String`, so this is `Clone` and [`Origin::name`] is a plain `fn`. Worth
//! it — the alternative is resolving a name against the library at every read,
//! which is exactly what makes a history unreadable after a rescan.
//!
//! # What is not built here
//!
//! **Only [`Origin::Playlist`] is constructed by the product today.** Every
//! other kind is built by [`Origin::decode`] and nowhere else, and that is the
//! honest state of ADR-0034: §2–§5 shipped (the origin on the command, the
//! marker in the ledger, the launch-time fold), while §1's `QueueVm::origin`
//! — which is what would make an album's run, a draw's and `All songs`' each
//! carry their own identity — is still `QueueVm::provenance`, a playlist name
//! or nothing. The decoder is general because a ledger written by a later baz
//! must stay readable by this one; the encoder is general because the two must
//! be one function or they will drift.
//!
//! **A kind with no lane subject must not be written as a marker until the
//! lane can credit it** ([`crate::lane::subject_of`]). Marking a run excludes
//! its plays from the records they quoted, so a marker whose kind the lane
//! throws away would lose the touch entirely. `no_kind_is_written_that_the_lane_cannot_credit`
//! is that rule, asserted rather than remembered.

/// The wire word for [`Origin::Album`].
const ALBUM: &str = "album";
/// The wire word for [`Origin::Artist`].
const ARTIST: &str = "artist";
/// The wire word for [`Origin::Playlist`].
const PLAYLIST: &str = "playlist";
/// The wire word for [`Origin::AllSongs`].
const ALL: &str = "all";
/// The wire word for [`Origin::Draw`].
const DRAW: &str = "draw";
/// The wire word for [`Origin::Hand`].
const HAND: &str = "hand";

/// **Which list this run is a reification of**, and the identity that kind of
/// list has.
///
/// The identity is a property of the **run**, not of the list object — which is
/// what reconciles this with `All songs`, a list that keeps no id, no path and
/// no `save`. Nothing is added to the list types; the queue built *from* one
/// carries an `Origin` that says which it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Origin {
    /// The implicit list every record is. `crate::vm::album_id`.
    Album {
        /// `crate::vm::album_id` of the record.
        id: u64,
        /// The record's title, as it was when the run played.
        name: String,
    },
    /// One artist's records, in the artist page's order. `crate::vm::artist_id`.
    Artist {
        /// `crate::vm::artist_id` of the artist.
        id: u64,
        /// The artist's name, as it was when the run played.
        name: String,
    },
    /// A playlist file — **the only kind with a file**, and so the only
    /// destination. [`crate::playlists::playlist_id`] over the name, which
    /// *is* the filename (ADR-0024 §2).
    Playlist {
        /// [`crate::playlists::playlist_id`] of the name.
        id: u64,
        /// The list's name — its file's stem, exactly.
        name: String,
    },
    /// **All songs** — the whole library, as the wall arranges it.
    ///
    /// Its identity is its **name and nothing else**: there is no file, no id,
    /// and nothing to look it up by, because there is only ever one of it.
    AllSongs,
    /// A shuffle draw: an order, not a place. Nothing durable — that is the
    /// finding rather than a gap. A draw is a list that existed once, and the
    /// run it produced is already the queue.
    Draw,
    /// Assembled one transfer at a time. **There was no list**, and saying so
    /// is different from saying nothing.
    ///
    /// Appending a record to a playlist's run makes a run that is no longer
    /// that playlist, so an append moves the run here carrying the name of
    /// what it was: *you started here and then made it your own* (ADR-0034
    /// §1.1). A list of origins was refused — it would make *which list am I
    /// in* a question with several answers, which is the thing the model
    /// exists to prevent.
    Hand {
        /// What the run was before it was made by hand, when it was anything.
        was: Option<String>,
    },
}

impl Origin {
    /// A run reified from the playlist file called `name`.
    ///
    /// The one constructor the product spends today, and the one gesture that
    /// has ever recorded provenance: `QueueVm::provenance` is *"the name of the
    /// playlist file this run was reified from"*, set in exactly one place
    /// (`playlists.rs`) and `None` everywhere else.
    pub(crate) fn playlist(name: &str) -> Self {
        Self::Playlist {
            id: crate::playlists::playlist_id(name),
            name: name.to_owned(),
        }
    }

    /// The list's name, in the listener's language.
    ///
    /// Sentence case like every other name in the product. `All songs` is the
    /// owner's own phrase, kept — and not *Everything*, which reads as a claim
    /// about the library rather than as a name for a list.
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Album { name, .. } | Self::Artist { name, .. } | Self::Playlist { name, .. } => {
                name
            }
            Self::AllSongs => "All songs",
            Self::Draw => "Shuffle",
            Self::Hand { was } => was.as_deref().unwrap_or("Queue"),
        }
    }

    /// **The playlist file this list is stored in** — `Some` for exactly one
    /// kind, by construction.
    ///
    /// The one method that makes the load-bearing property a fact about the
    /// type rather than a convention: a list with no file has nothing for the
    /// picker to append to, so `Add to "All songs"` is unrepresentable rather
    /// than merely absent.
    pub(crate) fn file(&self) -> Option<&str> {
        match self {
            Self::Playlist { name, .. } => Some(name),
            _ => None,
        }
    }

    /// **Whether this list is somewhere a track can be *added*.**
    ///
    /// The bit that stops a general origin re-opening the trap a bare
    /// `provenance.is_some()` used to hold shut by accident: every consumer
    /// that asks this gets the identical answer for every state reachable
    /// today, because there is exactly one `yes` in the column and it is the
    /// one that already had a file (ADR-0034 §1.3).
    ///
    /// > **A list you can be *in* is not the same as a list you can add *to*.
    /// > Only a file is a destination.**
    pub(crate) fn is_destination(&self) -> bool {
        self.file().is_some()
    }

    /// The identity this kind carries, when it carries one that can be joined
    /// on. `None` for the kinds whose identity is their name, or nothing.
    pub(crate) fn key(&self) -> Option<u64> {
        match self {
            Self::Album { id, .. } | Self::Artist { id, .. } | Self::Playlist { id, .. } => {
                Some(*id)
            }
            Self::AllSongs | Self::Draw | Self::Hand { .. } => None,
        }
    }

    /// The wire word for this kind — the first field of [`Self::encode`].
    fn kind(&self) -> &'static str {
        match self {
            Self::Album { .. } => ALBUM,
            Self::Artist { .. } => ARTIST,
            Self::Playlist { .. } => PLAYLIST,
            Self::AllSongs => ALL,
            Self::Draw => DRAW,
            Self::Hand { .. } => HAND,
        }
    }

    /// What the third field carries.
    ///
    /// [`Self::name`] for every kind whose name *is* its display, and the
    /// `was` text for [`Self::Hand`] — whose name is a word for the absence of
    /// a list and would not survive being read back as one.
    fn display(&self) -> &str {
        match self {
            Self::Hand { was } => was.as_deref().unwrap_or_default(),
            _ => self.name(),
        }
    }

    /// **This origin as one line a human can grep** (ADR-0034 §3):
    /// `<kind>:<key>:<display>`.
    ///
    /// ```text
    /// album:9c4f1a02bb37e5d1:Ochre
    /// playlist:3b1f00c2a49d7e60:Road Trip
    /// artist:57ea9b1103cc2fd4:Talk Talk
    /// all::All songs
    /// draw::Shuffle
    /// hand::from Road Trip
    /// ```
    ///
    /// The key is lowercase hex, empty where the kind has none. The display is
    /// last so that a name holding a colon needs no escaping — [`Self::decode`]
    /// splits on the **first two** only. Tabs and newlines are escaped by the
    /// ledger, in the one escaping vocabulary this product has
    /// (`baz_core::history`), rather than being invented a second time here.
    pub(crate) fn encode(&self) -> String {
        match self.key() {
            Some(key) => format!("{}:{key:x}:{}", self.kind(), self.display()),
            None => format!("{}::{}", self.kind(), self.display()),
        }
    }

    /// Read an origin back, or `None` for one this baz does not understand.
    ///
    /// **An unknown `kind` word is `None` — *we do not know* — rather than an
    /// error**, so a ledger written by a later baz stays readable by this one.
    /// That is the same answer a run with no marker at all gets, and it is the
    /// honest one: the run happened, and this baz cannot say what it came from.
    pub(crate) fn decode(text: &str) -> Option<Self> {
        let (kind, rest) = text.split_once(':')?;
        let (key, display) = rest.split_once(':')?;
        let id = || u64::from_str_radix(key, 16).ok();
        Some(match kind {
            ALBUM => Self::Album {
                id: id()?,
                name: display.to_owned(),
            },
            ARTIST => Self::Artist {
                id: id()?,
                name: display.to_owned(),
            },
            PLAYLIST => Self::Playlist {
                id: id()?,
                name: display.to_owned(),
            },
            ALL => Self::AllSongs,
            DRAW => Self::Draw,
            HAND => Self::Hand {
                was: (!display.is_empty()).then(|| display.to_owned()),
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every kind, for the sweeps below. Adding a variant without
    /// adding it here fails the exhaustiveness match in `every_kind`.
    fn every_kind() -> Vec<Origin> {
        // A match rather than a bare list, so a seventh kind cannot be added
        // without this file being opened.
        let named: Vec<Origin> = vec![
            Origin::Album {
                id: 0x9c4f_1a02_bb37_e5d1,
                name: "Ochre".to_owned(),
            },
            Origin::Artist {
                id: 0x57ea_9b11_03cc_2fd4,
                name: "Talk Talk".to_owned(),
            },
            Origin::playlist("Road Trip"),
            Origin::AllSongs,
            Origin::Draw,
            Origin::Hand {
                was: Some("from Road Trip".to_owned()),
            },
            Origin::Hand { was: None },
        ];
        for origin in &named {
            match origin {
                Origin::Album { .. }
                | Origin::Artist { .. }
                | Origin::Playlist { .. }
                | Origin::AllSongs
                | Origin::Draw
                | Origin::Hand { .. } => {}
            }
        }
        named
    }

    /// **The lines the ADR quotes, byte for byte.** The encoding is a file the
    /// owner is invited to `grep`; a change here is a change to what he reads.
    #[test]
    fn the_documented_encoding_is_the_encoding_that_is_written() {
        assert_eq!(
            Origin::Album {
                id: 0x9c4f_1a02_bb37_e5d1,
                name: "Ochre".to_owned()
            }
            .encode(),
            "album:9c4f1a02bb37e5d1:Ochre"
        );
        assert_eq!(
            Origin::Artist {
                id: 0x57ea_9b11_03cc_2fd4,
                name: "Talk Talk".to_owned()
            }
            .encode(),
            "artist:57ea9b1103cc2fd4:Talk Talk"
        );
        assert_eq!(Origin::AllSongs.encode(), "all::All songs");
        assert_eq!(Origin::Draw.encode(), "draw::Shuffle");
        assert_eq!(
            Origin::Hand {
                was: Some("from Road Trip".to_owned())
            }
            .encode(),
            "hand::from Road Trip"
        );
        // A playlist's key is its name's, which is ADR-0024 §2's rule: the
        // filename is the name, so the name is the identity.
        let road = Origin::playlist("Road Trip");
        assert_eq!(
            road.encode(),
            format!(
                "playlist:{:x}:Road Trip",
                crate::playlists::playlist_id("Road Trip")
            )
        );
    }

    #[test]
    fn every_kind_round_trips_through_its_own_line() {
        for origin in every_kind() {
            let line = origin.encode();
            assert_eq!(Origin::decode(&line), Some(origin.clone()), "{line}");
            // Exactly two colons are structural; the rest belong to the name.
            assert!(line.split(':').count() >= 3, "{line}");
        }
    }

    /// **A display name holding a colon survives untouched** — the reason the
    /// split is on the first two and not on all of them.
    #[test]
    fn a_name_holding_a_colon_survives() {
        for name in [
            "Side A: the long one",
            "10:04",
            "::",
            "a:b:c:d",
            "  leading and trailing  ",
        ] {
            let origin = Origin::playlist(name);
            assert_eq!(
                Origin::decode(&origin.encode()),
                Some(origin.clone()),
                "{name}"
            );
            assert_eq!(
                Origin::decode(&origin.encode()).expect("decodes").name(),
                name
            );
        }
    }

    /// **An origin a later baz wrote is `None`, not an error** — the property
    /// that keeps this baz reading a ledger it did not write.
    #[test]
    fn an_unknown_or_damaged_origin_is_simply_not_known() {
        for text in [
            "",
            "playlist",
            "playlist:",
            "moodboard:ff:Rainy Tuesday",
            "PLAYLIST:1f:Road Trip", // the wire words are lowercase
            "album:not-hex:Ochre",
            "album::Ochre", // a kind that must have a key, without one
            "artist::Talk Talk",
        ] {
            assert_eq!(Origin::decode(text), None, "{text:?}");
        }
    }

    /// **Exactly one kind is a destination**, and it is the one with a file.
    /// ADR-0024 §1 restated rather than amended — swept over every kind, so
    /// that a seventh cannot arrive without answering the question.
    #[test]
    fn only_a_file_is_a_destination() {
        for origin in every_kind() {
            assert_eq!(
                origin.is_destination(),
                origin.file().is_some(),
                "{origin:?} disagreed with itself"
            );
            assert_eq!(
                origin.is_destination(),
                matches!(origin, Origin::Playlist { .. }),
                "{origin:?} answered the wrong side of ADR-0024 §1"
            );
            assert!(!origin.name().is_empty(), "{origin:?} has no name");
        }
        assert_eq!(Origin::playlist("Road Trip").file(), Some("Road Trip"));
    }

    /// **A rename mints a new identity**, which is what lets the lane's row
    /// and the ledger's run find each other without a second key.
    #[test]
    fn two_lists_with_different_names_are_two_lists() {
        assert_ne!(
            Origin::playlist("Road Trip"),
            Origin::playlist("Late Shift")
        );
        assert_ne!(
            Origin::playlist("Road Trip").key(),
            Origin::playlist("Late Shift").key()
        );
    }

    /// **Nothing is written as a marker that the lane cannot credit.**
    ///
    /// Marking a run excludes its plays from the records they quoted, so a
    /// marker whose kind [`crate::lane::subject_of`] throws away would *lose*
    /// the touch rather than move it — a record played through `All songs`
    /// would vanish from the lane instead of rising in it.
    ///
    /// Asserted over this module's own source, because the property is about
    /// which constructors exist: `Origin::playlist` is the only one, so
    /// `Playlist` is the only kind that reaches a ledger, and it has a subject.
    /// A second constructor added without a subject to go with it fails here.
    #[test]
    fn no_kind_is_written_that_the_lane_cannot_credit() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/origin.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let start = source.find("impl Origin {").expect("the impl exists");
        let body = &source[start..source.find("\n#[cfg(test)]").expect("the impl ends")];
        let constructors: Vec<&str> = body
            .lines()
            .filter(|line| line.trim_start().starts_with("pub(crate) fn "))
            .filter(|line| line.contains("-> Self"))
            .collect();
        assert_eq!(
            constructors.len(),
            1,
            "a kind gained a constructor: give it a lane subject, or say here \
             why a run of it must never be marked\n{constructors:#?}"
        );
        assert!(
            constructors[0].contains("fn playlist("),
            "{constructors:#?}"
        );
        assert!(
            crate::lane::subject_of(&Origin::playlist("Road Trip")).is_some(),
            "the one kind the product writes has no lane row to credit"
        );
    }

    /// The kinds whose identity is their name carry no key, and the encoding
    /// says so with an empty field rather than a zero.
    #[test]
    fn a_kind_with_no_identity_writes_an_empty_key() {
        for origin in [Origin::AllSongs, Origin::Draw, Origin::Hand { was: None }] {
            assert_eq!(origin.key(), None, "{origin:?}");
            assert!(origin.encode().contains("::"), "{}", origin.encode());
        }
    }
}
