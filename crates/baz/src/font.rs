//! The bundled typeface: **IBM Plex**, compiled into the binary and installed
//! as the application's default family.
//!
//! This module is the asset half of [`crate::theme`]'s type scale. It holds the
//! bytes and the family names; `theme` turns them into the [`iced::Font`]
//! tokens the views ask for, and [`crate::app::run`] hands the bytes to
//! `iced::application(…).font(…)` at startup. Nothing here draws.
//!
//! # Why baz ships a typeface at all
//!
//! `iced::Font::DEFAULT` is `Family::SansSerif` — a *generic* family that each
//! platform resolves for itself — and baz then asks that unknown family for
//! `Weight::Medium` and `Weight::Semibold`. When the resolved family has no
//! such face the fallback lands somewhere else entirely: on the machine that
//! wrote `docs/design/02-visual-language.md` it landed on a **monospace**, so
//! every shelf tile's title was typewritten while the artist line directly
//! beneath it was proportional, and the product's one line of copy — *Where's
//! your music?* — was set in a typewriter face.
//!
//! That is a correctness problem before it is a taste problem: without a
//! bundled family baz is a different product on every machine.
//! [`crate::icon`] already refused system glyphs on exactly this ground ("a
//! player should look the same everywhere") and paid for the refusal with a
//! hand-written rasterizer. The same argument, applied to the whole interface's
//! voice, is worth an asset.
//!
//! # What is bundled, and what it costs
//!
//! Four faces, verbatim from upstream. They are **not subset**: OFL-1.1 §3
//! forbids a modified copy from using the Reserved Font Name, a subset *is* a
//! modified copy, and baz also renders other people's tags — the complete faces
//! carry Greek and Cyrillic, which a Latin subset would push back onto whatever
//! the host machine has. The measured trade, the provenance hashes, and the OFL
//! obligations are in `assets/fonts/README.md`.
//!
//! Codepoints Plex does not carry — CJK, Hebrew, Arabic, and the rest — still
//! fall back to the platform's fonts, exactly as they do today. Bundling
//! guarantees the glyphs baz itself draws, not every glyph a tag can hold.
//!
//! # There is no monospace, and there never needed to be one
//!
//! baz used to bundle Plex Mono for one job: iced 0.13 exposes no OpenType
//! feature control, so there is no `tnum` to ask for, and the conclusion drawn
//! was that every figure which changes in place had to be set in a monospaced
//! face. The premise is true; the conclusion does not follow.
//!
//! **IBM Plex Sans ships tabular figures by default.** Every digit advances
//! exactly 600/1000 em in Regular, Medium *and* `SemiBold` — the same advance
//! Plex Mono gave, with no kerning between digits and no default-on
//! substitution that touches them. `0:00:00` and `9:59:59` both measure
//! 43.008 px at [`crate::theme::SIZE_META`], to 0.000 px.
//! `the_sans_carries_baz_s_tabular_figures_in_every_weight_it_sets_them_in`
//! measures that against these very bytes, and it is the licence for the
//! deletion (`docs/design/02-visual-language.md` §3.1, `.interface-design/system.md` §8).
//!
//! Deleting it also *fixed* something: `theme::STAMP_W` is 52 px, and
//! `10:00:00` measures 57.60 px in Plex Mono — the shipped build could not
//! hold a ten-hour track — against 50.21 px in Plex Sans, which it can. The
//! slot test below now includes that string.
//!
//! # The risk this module is accountable for
//!
//! Every fixed-width slot in the pixel-stable bottom bar is sized against a
//! face's real advances, and `theme.rs`'s cheap compile-time guards can only
//! bound the *figures* in a string once the face is proportional everywhere
//! else. The tests below therefore parse these very bytes and measure the real
//! advance width of each worst-case string against the token that reserves room
//! for it, rather than trusting an em-fraction.
//! `docs/design/02-visual-language.md` §3.4 asks for this test by name and says
//! not to ship a face change without it.

/// The bundled sans family, as the `name` table spells it. This is the string
/// `Font::with_name` must be given for the Regular, Medium and `SemiBold` faces
/// to resolve as one family at three weights.
pub const SANS: &str = "IBM Plex Sans";
/// The bundled serif family: the album title and the first-run question, and
/// nothing else (`docs/design/02-visual-language.md` §2.2.3).
pub const SERIF: &str = "IBM Plex Serif";

/// IBM Plex Sans Regular — body text.
pub const SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
/// IBM Plex Sans Medium — tile titles, control labels, the playing row.
pub const SANS_MEDIUM: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf");
/// IBM Plex Sans `SemiBold` — headings, and the primary action's label.
pub const SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");
/// IBM Plex Serif `SemiBold` — the album title and the first-run question.
pub const SERIF_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSerif-SemiBold.ttf");

/// Every bundled face, in the order the application loads them.
///
/// Loading is one pass at startup: `iced::application(…).font(bytes)` per
/// entry, before the window exists. The bytes are `'static` slices of the
/// binary's own rodata, so each `Cow` iced takes is borrowed and nothing is
/// copied or read from disk.
pub const FACES: [&[u8]; 4] = [SANS_REGULAR, SANS_MEDIUM, SANS_SEMIBOLD, SERIF_SEMIBOLD];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    /// The smallest TrueType reader that can answer "how wide is this string".
    ///
    /// Four tables — `head` for the em square, `hhea`/`hmtx` for advances,
    /// `cmap` for codepoint-to-glyph — and no dependency. A shaping engine
    /// would add kerning and ligatures on top of this; neither applies to the
    /// figures in baz's fixed slots (Plex Sans's digits are uniform-advance and
    /// pair no kerns with each other), so the sum of advances *is* the rendered
    /// width for everything measured here, and is an upper bound elsewhere
    /// because kerning only ever pulls glyphs together.
    ///
    /// Deliberately hand-written and test-only rather than a `ttf-parser`
    /// dev-dependency: the crate is already flagged unmaintained in
    /// `deny.toml`, and this is 120 lines against a file we ship and hash.
    mod ttf {
        /// A parsed face: enough of one to measure text.
        pub struct Face<'a> {
            data: &'a [u8],
            /// Units per em, from `head`.
            upem: f32,
            /// Offset of the `hmtx` table.
            hmtx: usize,
            /// `numberOfHMetrics` from `hhea` — glyphs past this share the
            /// last entry's advance (the monospace tail of a proportional
            /// font, and the whole of a monospaced one).
            long_metrics: usize,
            /// Offset of the chosen `cmap` subtable.
            cmap: usize,
        }

        fn be16(data: &[u8], at: usize) -> u16 {
            u16::from_be_bytes([data[at], data[at + 1]])
        }

        fn be32(data: &[u8], at: usize) -> u32 {
            u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
        }

        /// A table offset from the file's table directory, by four-byte tag.
        fn table(data: &[u8], tag: [u8; 4]) -> usize {
            let count = usize::from(be16(data, 4));
            for index in 0..count {
                let entry = 12 + 16 * index;
                if data[entry..entry + 4] == tag {
                    return be32(data, entry + 8) as usize;
                }
            }
            panic!(
                "the face carries no {} table",
                String::from_utf8_lossy(&tag)
            );
        }

        impl<'a> Face<'a> {
            /// Parse `data`, or panic saying which table was unreadable.
            pub fn parse(data: &'a [u8]) -> Self {
                let head = table(data, *b"head");
                let hhea = table(data, *b"hhea");
                Self {
                    data,
                    upem: f32::from(be16(data, head + 18)),
                    hmtx: table(data, *b"hmtx"),
                    long_metrics: usize::from(be16(data, hhea + 34)),
                    cmap: subtable(data, table(data, *b"cmap")),
                }
            }

            /// Units per em — the denominator every advance is scaled by.
            pub fn units_per_em(&self) -> f32 {
                self.upem
            }

            /// The glyph id `codepoint` maps to, or 0 (`.notdef`) when the
            /// face has no glyph for it.
            pub fn glyph(&self, codepoint: char) -> u16 {
                let code = codepoint as u32;
                match be16(self.data, self.cmap) {
                    4 => self.glyph_format4(code),
                    12 => self.glyph_format12(code),
                    other => panic!("unsupported cmap format {other}"),
                }
            }

            /// `codepoint`'s advance width in em units.
            pub fn advance(&self, codepoint: char) -> f32 {
                let glyph = usize::from(self.glyph(codepoint));
                let index = glyph.min(self.long_metrics - 1);
                f32::from(be16(self.data, self.hmtx + 4 * index))
            }

            /// The width `text` occupies when set at `size` logical pixels:
            /// the sum of its glyphs' advances, scaled out of em units.
            pub fn width(&self, text: &str, size: f32) -> f32 {
                text.chars()
                    .map(|character| self.advance(character))
                    .sum::<f32>()
                    * size
                    / self.upem
            }

            fn glyph_format4(&self, code: u32) -> u16 {
                let Ok(code) = u16::try_from(code) else {
                    return 0;
                };
                let base = self.cmap;
                let segments = usize::from(be16(self.data, base + 6)) / 2;
                let ends = base + 14;
                let starts = ends + 2 * segments + 2;
                let deltas = starts + 2 * segments;
                let ranges = deltas + 2 * segments;
                for segment in 0..segments {
                    let end = be16(self.data, ends + 2 * segment);
                    let start = be16(self.data, starts + 2 * segment);
                    if code > end || code < start {
                        continue;
                    }
                    let delta = be16(self.data, deltas + 2 * segment);
                    let range = be16(self.data, ranges + 2 * segment);
                    if range == 0 {
                        return code.wrapping_add(delta);
                    }
                    let at =
                        ranges + 2 * segment + usize::from(range) + 2 * usize::from(code - start);
                    let glyph = be16(self.data, at);
                    return if glyph == 0 {
                        0
                    } else {
                        glyph.wrapping_add(delta)
                    };
                }
                0
            }

            fn glyph_format12(&self, code: u32) -> u16 {
                let base = self.cmap;
                let groups = be32(self.data, base + 12) as usize;
                for group in 0..groups {
                    let at = base + 16 + 12 * group;
                    let start = be32(self.data, at);
                    let end = be32(self.data, at + 4);
                    if code >= start && code <= end {
                        let glyph = be32(self.data, at + 8) + (code - start);
                        return u16::try_from(glyph).unwrap_or(0);
                    }
                }
                0
            }
        }

        /// The best Unicode `cmap` subtable in the table at `cmap`: a full
        /// (3, 10) one if the face has it, else the (3, 1) BMP one, else any
        /// Unicode platform subtable.
        fn subtable(data: &[u8], cmap: usize) -> usize {
            let count = usize::from(be16(data, cmap + 2));
            let mut chosen = None;
            for index in 0..count {
                let record = cmap + 4 + 8 * index;
                let platform = be16(data, record);
                let encoding = be16(data, record + 2);
                let offset = cmap + be32(data, record + 4) as usize;
                let rank = match (platform, encoding) {
                    (3, 10) => 3,
                    (3, 1) => 2,
                    (0, _) => 1,
                    _ => continue,
                };
                if chosen.is_none_or(|(best, _)| rank > best) {
                    chosen = Some((rank, offset));
                }
            }
            chosen.expect("the face carries no Unicode cmap subtable").1
        }
    }

    use ttf::Face;

    /// The sans face at Regular: baz's whole voice, and every figure it sets
    /// in a reserved slot.
    fn sans() -> Face<'static> {
        Face::parse(SANS_REGULAR)
    }

    /// How much room a slot must keep past the string it reserves for.
    ///
    /// One logical pixel. Not a safety fudge for an unknown face — the face is
    /// shipped, hashed and measured here — but the difference between a slot
    /// that holds its worst case and a slot that is exactly full, which is one
    /// rounding decision inside the renderer away from clipping.
    const SLACK: f32 = 1.0;

    /// `slot` holds `text` set at `size` in `face`, with room to spare.
    #[track_caller]
    fn fits(face: &Face<'_>, text: &str, size: f32, slot: f32, token: &str) {
        let width = face.width(text, size);
        assert!(
            width + SLACK <= slot,
            "{token} reserves {slot} px; {text:?} at {size} px measures \
             {width:.2} px in the bundled face, leaving {:.2} px",
            slot - width
        );
    }

    #[test]
    fn every_bundled_face_parses_and_shares_one_em_square() {
        // If a face were swapped for one with a different em square, every
        // measurement below would silently change its meaning.
        for (name, bytes) in [
            ("Sans Regular", SANS_REGULAR),
            ("Sans Medium", SANS_MEDIUM),
            ("Sans SemiBold", SANS_SEMIBOLD),
            ("Serif SemiBold", SERIF_SEMIBOLD),
        ] {
            let face = Face::parse(bytes);
            assert!(
                (face.units_per_em() - 1000.0).abs() < f32::EPSILON,
                "{name} is drawn on a {} unit em square, not 1000",
                face.units_per_em()
            );
        }
        assert_eq!(
            FACES.len(),
            4,
            "the monospace is deleted (`.interface-design/system.md` §8)"
        );
    }

    /// Every character baz can put in a fixed slot has a real glyph in the
    /// face that will be asked to draw it.
    ///
    /// A missing glyph is not a crash: cosmic-text falls back to whatever the
    /// host has, which is precisely the "different product on every machine"
    /// this module exists to end. The set below is every character baz's own
    /// formatters can emit — timestamps, levels, rates, counts, the minus sign
    /// the stepper uses, and the separators the captions use.
    #[test]
    fn the_sans_face_carries_every_figure_baz_sets_in_it() {
        let sans = sans();
        for character in "0123456789:.,-+ dBkHz∞→−·…—".chars() {
            assert_ne!(
                sans.glyph(character),
                0,
                "IBM Plex Sans has no glyph for {character:?} — it would fall \
                 back to a system font mid-readout"
            );
        }
        // Every weight the readouts and their labels can be set in, and the
        // serif that still carries the album title.
        for face in [
            Face::parse(SANS_MEDIUM),
            Face::parse(SANS_SEMIBOLD),
            Face::parse(SERIF_SEMIBOLD),
        ] {
            for character in "…—·’“”→".chars() {
                assert_ne!(face.glyph(character), 0, "no glyph for {character:?}");
            }
        }
    }

    /// **The measurement that deleted the monospace.**
    ///
    /// baz shipped a second face for one reason: iced 0.13 exposes no
    /// OpenType feature control, so there is no `tnum`, and a column of figures
    /// that ticks must not move its neighbours. That reasoning skipped a step —
    /// it never asked whether the *sans* already had tabular figures. It does,
    /// in all three weights, at the same 600/1000 em the mono gave.
    ///
    /// This is the licence for the monospace's deletion, taken against the bytes baz
    /// ships rather than against a foundry's specimen page, so a face swapped
    /// for one with proportional figures fails here rather than in a user's
    /// bottom bar.
    #[test]
    fn the_sans_carries_baz_s_tabular_figures_in_every_weight_it_sets_them_in() {
        for (name, bytes) in [
            ("Sans Regular", SANS_REGULAR),
            ("Sans Medium", SANS_MEDIUM),
            ("Sans SemiBold", SANS_SEMIBOLD),
        ] {
            let face = Face::parse(bytes);
            let reference = face.advance('0');
            for digit in "0123456789".chars() {
                assert!(
                    (face.advance(digit) - reference).abs() < f32::EPSILON,
                    "{name}: {digit:?} advances {} where '0' advances {reference} \
                     — the figures are not tabular and a ticking readout will jitter",
                    face.advance(digit)
                );
            }
            // The same 0.6 em the mono advanced at, which is what makes this a
            // substitution rather than a re-derivation of every reserved slot.
            assert!(
                (reference / face.units_per_em() - 0.6).abs() < 0.001,
                "{name}'s digit advance moved off 0.6 em"
            );
        }

        // And end to end, in the strings that actually tick: a timestamp
        // rolling from all-zeroes to all-nines moves nothing beside it.
        let sans = sans();
        for (left, right) in [
            ("0:00:00", "9:59:59"),
            ("1:23:45", "8:07:02"),
            ("999", "111"),
            ("199 / 240", "999 / 999"),
        ] {
            let (a, b) = (
                sans.width(left, theme::SIZE_META),
                sans.width(right, theme::SIZE_META),
            );
            assert!(
                (a - b).abs() < f32::EPSILON,
                "{left:?} measures {a} px and {right:?} measures {b} px"
            );
        }
    }

    /// **The advance-width test** `docs/design/02-visual-language.md` §3.4
    /// requires before a face change may ship.
    ///
    /// The bottom bar is pixel-stable because nothing in it is sized to its
    /// content: every slot is a token wide enough for its worst case. Those
    /// tokens were chosen against a face, and a different face has different
    /// figure widths. Each assertion here takes the worst-case string the
    /// token's own documentation names, measures it in the face that will
    /// actually draw it, and checks the reservation against the measurement —
    /// so a font change can never silently overflow a reserved slot, it can
    /// only fail this test.
    ///
    /// **Every slot is now measured in the Sans**, which is the whole of what
    /// deleting the monospace cost: one face draws every string in the bar.
    #[test]
    fn every_reserved_slot_holds_its_worst_case_in_the_bundled_face() {
        let sans = sans();

        // The seek bar's timestamps: `h:mm:ss`, the widest shape
        // `vm::format_duration` produces for a track — including the ten-hour
        // case the *mono* could not hold in this slot (57.60 px against 52).
        for stamp in ["0:00:00", "10:00:00"] {
            fits(&sans, stamp, theme::SIZE_META, theme::STAMP_W, "STAMP_W");
        }
        // …and the undeclared-length placeholder, which shares the slot.
        fits(&sans, "--:--", theme::SIZE_META, theme::STAMP_W, "STAMP_W");

        // The signal-path readout: the longest chain a consumer device
        // produces, and the affirmative reading that shares its slot.
        for note in ["192 → 176.4 kHz", "44.1 → 48 kHz", "bit-perfect"] {
            fits(&sans, note, theme::SIZE_META, theme::SIGNAL_W, "SIGNAL_W");
        }

        // The volume level tip: `player::level_label`'s widest output, and the
        // silent end of the taper, which uses a different glyph entirely.
        for level in ["-18.1 dB", "-60.0 dB", "-∞ dB"] {
            fits(&sans, level, theme::SIZE_CAPTION, theme::LEVEL_W, "LEVEL_W");
        }

        // The seek preview tip, one size down from the stamps.
        fits(
            &sans,
            "0:00:00",
            theme::SIZE_CAPTION,
            theme::PREVIEW_W,
            "PREVIEW_W",
        );

        // A setting's value slot: `replaygain::format_centidb` at either end
        // of the pre-amp travel.
        for value in ["-20.00 dB", "+20.00 dB"] {
            fits(
                &sans,
                value,
                theme::SIZE_META,
                theme::SETTING_VALUE_W,
                "SETTING_VALUE_W",
            );
        }

        // The track/queue number column: three figures, per its own docs.
        fits(
            &sans,
            "999",
            theme::SIZE_META,
            theme::TRACK_NO_W,
            "TRACK_NO_W",
        );

        // The bar's queue-position readout, which reserves for a queue nobody
        // bounded: `999 / 999` is the widest it can be asked to draw.
        fits(
            &sans,
            "999 / 999",
            theme::SIZE_META,
            theme::QUEUE_POS_W,
            "QUEUE_POS_W",
        );

        // The bar's Up next control: its label in the Medium face it is set
        // in, inside what the slot has left after the readout and the padding.
        fits(
            &Face::parse(SANS_MEDIUM),
            "Up next",
            theme::SIZE_META,
            theme::UP_NEXT_W - theme::QUEUE_POS_W - 3.0 * theme::GAP_SM,
            "UP_NEXT_W",
        );

        // The Settings place's section list: the longest name any of the
        // sections §4.5 plans can have, in the face and size the list draws
        // them at, inside the padding the entry carries.
        for section in ["Playback", "Appearance", "Library", "About"] {
            fits(
                &Face::parse(SANS_MEDIUM),
                section,
                theme::SIZE_BODY,
                theme::SETTINGS_NAV_W - 2.0 * theme::GAP_MD,
                "SETTINGS_NAV_W",
            );
        }

        // The top bar's one remaining control, in the Medium face it is set in
        // and inside the padding it carries. It used to be sized to the *queue*
        // toggle beside it, and wrapped to two lines at a 760 px window
        // because of it; now it is sized to its own word, which only means
        // anything if the word is measured.
        fits(
            &Face::parse(SANS_MEDIUM),
            "Settings",
            theme::SIZE_META,
            theme::SETTINGS_TOGGLE_W - 2.0 * theme::GAP_SM,
            "SETTINGS_TOGGLE_W",
        );
    }

    /// The settings panel's reserved note slot still holds every sentence it
    /// can be asked to show — measured in the bundled Sans and wrapped the way
    /// the toolkit wraps, rather than bounded by an average character width.
    ///
    /// This is the same claim `theme.rs`'s `a_setting_note_fits_the_slot_it_is_given`
    /// makes with arithmetic; here it is made with the face's own metrics,
    /// because the slot is two lines tall and a wider face is exactly what
    /// pushes a sentence onto a third.
    #[test]
    fn a_setting_note_still_wraps_inside_its_two_reserved_lines() {
        use crate::replaygain::{MODES, mode_note};

        let sans = sans();
        // The width a wrapped line actually has: the panel, less its inset on
        // both sides, less the scrollbar lane the list keeps clear.
        let content_w = theme::PANEL_W - 2.0 * theme::GAP_XL - theme::SCROLLBAR_LANE;
        let lines = theme::SETTING_NOTE_H / (theme::SIZE_META * theme::LINE_HEIGHT);
        for mode in MODES {
            let note = mode_note(mode);
            let used = wrapped_lines(&sans, note, theme::SIZE_META, content_w);
            assert!(
                used <= lines,
                "{note:?} wraps to {used} lines in {content_w} px of the \
                 bundled Sans; the reserved slot is {lines} lines tall"
            );
        }
    }

    /// How many lines `text` takes when greedily wrapped at word boundaries
    /// into `width` — the algorithm cosmic-text uses for `Wrapping::Word`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a line count for one sentence is far below f32's exact-integer range"
    )]
    fn wrapped_lines(face: &Face<'_>, text: &str, size: f32, width: f32) -> f32 {
        let space = face.width(" ", size);
        let mut lines = 1_u32;
        let mut used = 0.0_f32;
        for word in text.split(' ') {
            let word_w = face.width(word, size);
            let needed = if used > 0.0 { space + word_w } else { word_w };
            if used + needed > width && used > 0.0 {
                lines += 1;
                used = word_w;
            } else {
                used += needed;
            }
        }
        lines as f32
    }
}
