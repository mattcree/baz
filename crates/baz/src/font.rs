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
//! Five faces, verbatim from upstream, 1 001 520 bytes in total. They are
//! **not subset**: OFL-1.1 §3 forbids a modified copy from using the Reserved
//! Font Name, a subset *is* a modified copy, and baz also renders other
//! people's tags — the complete faces carry Greek and Cyrillic, which a Latin
//! subset would push back onto whatever the host machine has. The measured
//! trade (a subset saves ~666 KB), the provenance hashes, and the OFL
//! obligations are in `assets/fonts/README.md`.
//!
//! Codepoints Plex does not carry — CJK, Hebrew, Arabic, and the rest — still
//! fall back to the platform's fonts, exactly as they do today. Bundling
//! guarantees the glyphs baz itself draws, not every glyph a tag can hold.
//!
//! # The risk this module is accountable for
//!
//! Plex Mono advances 600/1000 em for *every* glyph, where `theme.rs`'s
//! reserved-slot assertions used to guess with 0.5 em — the new face is 20%
//! wider than the old assumption, and every fixed-width slot in the
//! pixel-stable bottom bar was sized against the old one. The tests below
//! therefore parse these very bytes and measure the real advance width of each
//! worst-case string against the token that reserves room for it, rather than
//! trusting an em-fraction. `docs/design/02-visual-language.md` §4.6 asks for
//! this test by name and says not to ship the font change without it.

/// The bundled sans family, as the `name` table spells it. This is the string
/// `Font::with_name` must be given for the Regular, Medium and `SemiBold` faces
/// to resolve as one family at three weights.
pub const SANS: &str = "IBM Plex Sans";
/// The bundled monospace family — baz's tabular figures (iced 0.13 has no
/// OpenType feature control, so there is no `tnum` to ask for instead).
pub const MONO: &str = "IBM Plex Mono";
/// The bundled serif family: the album title and the first-run question, and
/// nothing else (`docs/design/02-visual-language.md` §2.2.3).
pub const SERIF: &str = "IBM Plex Serif";

/// IBM Plex Sans Regular — body text.
pub const SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
/// IBM Plex Sans Medium — tile titles, control labels, the playing row.
pub const SANS_MEDIUM: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf");
/// IBM Plex Sans `SemiBold` — headings, and the primary action's label.
pub const SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");
/// IBM Plex Mono Regular — every figure that changes in place.
pub const MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");
/// IBM Plex Serif `SemiBold` — the album title and the first-run question.
pub const SERIF_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSerif-SemiBold.ttf");

/// Every bundled face, in the order the application loads them.
///
/// Loading is one pass at startup: `iced::application(…).font(bytes)` per
/// entry, before the window exists. The bytes are `'static` slices of the
/// binary's own rodata, so each `Cow` iced takes is borrowed and nothing is
/// copied or read from disk.
pub const FACES: [&[u8]; 5] = [
    SANS_REGULAR,
    SANS_MEDIUM,
    SANS_SEMIBOLD,
    MONO_REGULAR,
    SERIF_SEMIBOLD,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    /// The smallest TrueType reader that can answer "how wide is this string".
    ///
    /// Four tables — `head` for the em square, `hhea`/`hmtx` for advances,
    /// `cmap` for codepoint-to-glyph — and no dependency. A shaping engine
    /// would add kerning and ligatures on top of this; neither applies to the
    /// figures in baz's fixed slots (Plex Mono is uniform-advance and pairs no
    /// kerns between digits), so the sum of advances *is* the rendered width
    /// for everything measured here, and is an upper bound elsewhere because
    /// kerning only ever pulls glyphs together.
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

    /// The mono face, parsed. Every figure baz sets in a reserved slot is in
    /// this one.
    fn mono() -> Face<'static> {
        Face::parse(MONO_REGULAR)
    }

    /// The sans face at Regular, which is what wraps in the settings note.
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
            ("Mono Regular", MONO_REGULAR),
            ("Serif SemiBold", SERIF_SEMIBOLD),
        ] {
            let face = Face::parse(bytes);
            assert!(
                (face.units_per_em() - 1000.0).abs() < f32::EPSILON,
                "{name} is drawn on a {} unit em square, not 1000",
                face.units_per_em()
            );
        }
        assert_eq!(FACES.len(), 5, "five faces, per the visual language §2.2.1");
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
    fn the_mono_face_carries_every_figure_baz_sets_in_it() {
        let mono = mono();
        for character in "0123456789:.,-+ dBkHz∞→−·…—".chars() {
            assert_ne!(
                mono.glyph(character),
                0,
                "IBM Plex Mono has no glyph for {character:?} — it would fall \
                 back to a system font mid-readout"
            );
        }
        // The serif and sans carry the punctuation the copy uses.
        for face in [sans(), Face::parse(SERIF_SEMIBOLD)] {
            for character in "…—·’“”→".chars() {
                assert_ne!(face.glyph(character), 0, "no glyph for {character:?}");
            }
        }
    }

    /// The mono really is monospaced, which is the whole reason baz sets its
    /// figures in it: a digit changing must not move its neighbour.
    #[test]
    fn the_mono_face_advances_uniformly() {
        let mono = mono();
        let reference = mono.advance('0');
        for character in "0123456789:.- dBkHz".chars() {
            assert!(
                (mono.advance(character) - reference).abs() < f32::EPSILON,
                "{character:?} advances {} where '0' advances {reference}",
                mono.advance(character)
            );
        }
        // The figure the old assertions guessed with was 0.5 em. The face
        // actually ships is 0.6 em — 20% wider — which is exactly why the
        // slot checks below measure rather than estimate.
        assert!(
            (reference / mono.units_per_em() - 0.6).abs() < 0.001,
            "Plex Mono's advance moved off 0.6 em"
        );
    }

    /// **The advance-width test** `docs/design/02-visual-language.md` §4.6
    /// requires before the typeface may ship.
    ///
    /// The bottom bar is pixel-stable because nothing in it is sized to its
    /// content: every slot is a token wide enough for its worst case. Those
    /// tokens were chosen against the platform face baz used to borrow, and a
    /// different face has different figure widths. Each assertion here takes
    /// the worst-case string the token's own documentation names, measures it
    /// in the face that will actually draw it, and checks the reservation
    /// against the measurement — so a font change can never silently overflow
    /// a reserved slot, it can only fail this test.
    #[test]
    fn every_reserved_slot_holds_its_worst_case_in_the_bundled_face() {
        let mono = mono();

        // The seek bar's timestamps: `h:mm:ss`, the widest shape
        // `vm::format_duration` produces for a track.
        fits(
            &mono,
            "0:00:00",
            theme::SIZE_META,
            theme::STAMP_W,
            "STAMP_W",
        );
        // …and the undeclared-length placeholder, which shares the slot.
        fits(&mono, "--:--", theme::SIZE_META, theme::STAMP_W, "STAMP_W");

        // The signal-path readout: the longest chain a consumer device
        // produces, and the affirmative reading that shares its slot.
        for note in ["192 → 176.4 kHz", "44.1 → 48 kHz", "bit-perfect"] {
            fits(&mono, note, theme::SIZE_META, theme::SIGNAL_W, "SIGNAL_W");
        }

        // The volume level tip: `player::level_label`'s widest output, and the
        // silent end of the taper, which uses a different glyph entirely.
        for level in ["-18.1 dB", "-60.0 dB", "-∞ dB"] {
            fits(&mono, level, theme::SIZE_CAPTION, theme::LEVEL_W, "LEVEL_W");
        }

        // The seek preview tip, one size down from the stamps.
        fits(
            &mono,
            "0:00:00",
            theme::SIZE_CAPTION,
            theme::PREVIEW_W,
            "PREVIEW_W",
        );

        // A setting's value slot: `replaygain::format_centidb` at either end
        // of the pre-amp travel.
        for value in ["-20.00 dB", "+20.00 dB"] {
            fits(
                &mono,
                value,
                theme::SIZE_META,
                theme::SETTING_VALUE_W,
                "SETTING_VALUE_W",
            );
        }

        // The track/queue number column: three figures, per its own docs.
        fits(
            &mono,
            "999",
            theme::SIZE_META,
            theme::TRACK_NO_W,
            "TRACK_NO_W",
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
