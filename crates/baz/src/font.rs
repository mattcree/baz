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
//! **One family at three weights** — Sans Regular, Medium and `SemiBold`,
//! 605 592 bytes — verbatim from upstream. They are **not subset**: OFL-1.1 §3
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
//! # There is no display face either
//!
//! Plex Serif `SemiBold` was bundled for two jobs — the album's title and the
//! first-run question — and revision 1 of the spec nominated it, in the same
//! paragraph that introduced it, as the first thing to cut if the design ever
//! needed disciplining. The gallery direction is that moment: the room supplies
//! nothing and the work supplies everything, and a display face is the room
//! supplying personality. The album title is Sans `SemiBold` at 22.
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

/// IBM Plex Sans Regular — body text.
pub const SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
/// IBM Plex Sans Medium — tile titles, control labels, the playing row.
pub const SANS_MEDIUM: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf");
/// IBM Plex Sans `SemiBold` — headings, and the primary action's label.
pub const SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");

/// **The serif italic**, and the one thing it is for: a *work's own title* on
/// the Home place's `CONTINUE` placard.
///
/// # Why a second family exists at all, after this module argued against one
///
/// It argued against a **display face** — a serif standing in for the room's
/// personality, in headings and questions, where the gallery direction says
/// the room supplies nothing and the work supplies everything. That argument
/// is unchanged and Plex Serif `SemiBold` stays deleted.
///
/// This is a different job. baz's identity is a gallery, and its icon is a
/// work under a wall label; on a museum placard the **work's title is set in
/// italic** and everything around it — the artist, the date, the medium — is
/// not. The italic is not decorating the placard, it is the placard's own
/// convention for saying *this string is the name of the thing, not a fact
/// about it*. Nothing else in the product is a work's title standing alone
/// beside its own facts, so nothing else takes it.
///
/// **The owner saw the risk and approved it** (2026-08-09). It is kept to one
/// token ([`crate::theme::WORK_TITLE`]) so it is one line to revert.
///
/// Same family as the bundled Sans, same licence, same upstream commit,
/// complete rather than subset — `assets/fonts/README.md` carries the hash and
/// the OFL obligations.
pub const SERIF_ITALIC: &[u8] = include_bytes!("../assets/fonts/IBMPlexSerif-Italic.ttf");

/// The serif family's name, for [`iced::Font::with_name`].
pub const SERIF: &str = "IBM Plex Serif";

/// Every bundled face, in the order the application loads them.
///
/// Loading is one pass at startup: `iced::application(…).font(bytes)` per
/// entry, before the window exists. The bytes are `'static` slices of the
/// binary's own rodata, so each `Cow` iced takes is borrowed and nothing is
/// copied or read from disk.
pub const FACES: [&[u8]; 4] = [SANS_REGULAR, SANS_MEDIUM, SANS_SEMIBOLD, SERIF_ITALIC];

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
            "the sans at three weights, and the serif italic — which is not a \
             display face (`.interface-design/system.md` §8's argument stands) \
             but the museum placard's own convention for a work's title, on \
             the one placard in the product ([`SERIF_ITALIC`])"
        );
        // The serif is drawn on the same em square as the sans, so the two
        // share one type scale rather than needing a second set of numbers.
        let serif = Face::parse(SERIF_ITALIC);
        assert!(
            (serif.units_per_em() - 1000.0).abs() < f32::EPSILON,
            "the serif italic is drawn on a {} unit em square, not 1000",
            serif.units_per_em()
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
        // Every weight the readouts, their labels and the album title can be
        // set in — which, with the serif deleted, is the whole bundle.
        for face in [Face::parse(SANS_MEDIUM), Face::parse(SANS_SEMIBOLD)] {
            for character in "…—·’“”→".chars() {
                assert_ne!(face.glyph(character), 0, "no glyph for {character:?}");
            }
        }
    }

    /// **The tracking is real, in every weight a heading can be set in.**
    ///
    /// [`theme::tracked`] spells letter-spacing into the string as U+2009 THIN
    /// SPACE, because iced 0.13 has no `letter-spacing` property. That is only
    /// honest if the bundled faces actually carry the character: a missing
    /// glyph would fall back to whatever the host has, which for a space-like
    /// character is invisible until it is not.
    ///
    /// Measured, not assumed: the advance is read out of `hmtx` and asserted to
    /// be a real, non-zero fraction of the em — 0.118 em in these faces, which
    /// is squarely inside the 0.08 – 0.15 em band caps at this size want. And
    /// the tracked string is asserted to be *wider* than the untracked one by
    /// exactly one advance per gap, which is the arithmetic the layout below
    /// depends on.
    #[test]
    fn the_bundled_faces_carry_the_tracking_space() {
        let track: char = theme::TRACKING
            .chars()
            .next()
            .expect("the tracking is one character");
        assert_eq!(track as u32, 0x2009, "the tracking is U+2009 THIN SPACE");
        for (name, bytes) in [
            ("Sans Regular", SANS_REGULAR),
            ("Sans Medium", SANS_MEDIUM),
            ("Sans SemiBold", SANS_SEMIBOLD),
        ] {
            let face = Face::parse(bytes);
            assert_ne!(
                face.glyph(track),
                0,
                "{name} has no glyph for the tracking space — a tracked \
                 heading would fall back to a system font between every pair \
                 of letters"
            );
            let em = face.advance(track) / face.units_per_em();
            assert!(
                (0.08..=0.15).contains(&em),
                "{name} advances the tracking space {em} em, which is not \
                 letter-spacing"
            );
            // One advance per *gap*, never a trailing one: a tracked label that
            // ended in a space would sit a pixel off a right-aligned edge.
            let plain = face.width("ARTIST", theme::SIZE_HEADING);
            let tracked = face.width(&theme::tracked("ARTIST"), theme::SIZE_HEADING);
            let gaps = 5.0 * face.width(theme::TRACKING, theme::SIZE_HEADING);
            assert!(
                (tracked - plain - gaps).abs() < 0.01,
                "{name}: tracked {tracked:.2} px against plain {plain:.2} px \
                 and {gaps:.2} px of track"
            );
        }
    }

    /// **The group-key row fits the shipped window**, tracked caps and all.
    ///
    /// The row is five words, each in a button with [`theme::GAP_XS`] of
    /// padding on both sides and [`theme::GAP_MD`] between them, sitting after
    /// the search well and the gap that separates the two clusters. The right
    /// of the bar holds the gear alone at rest. Measured against the 1280 px
    /// window baz opens at, with the well at its 280 px ceiling — and the
    /// counts, which now live *inside* the well (doc 10 §7 step 2), are
    /// measured against the well's own text lane rather than against the
    /// strip.
    #[test]
    fn the_group_key_row_fits_the_top_bar_at_the_shipped_window() {
        use baz_core::index::GroupKey;

        let medium = Face::parse(SANS_MEDIUM);
        let keys: f32 = GroupKey::ALL
            .iter()
            .map(|key| {
                medium.width(
                    &theme::tracked(&key.label().to_uppercase()),
                    theme::SIZE_META,
                ) + 2.0 * theme::GAP_XS
            })
            .sum::<f32>()
            + 4.0 * theme::GAP_MD;
        // **At the shipped window the strip has no well**: the lane holds it
        // (ADR-0030's search amendment), so the strip's left cluster is the
        // states and the acts, hanging from the window gutter, and the strip's
        // own width is the window less the expanded lane.
        assert!(
            !theme::strip_holds_the_well(1280.0),
            "at the shipped window the well is the lane's"
        );
        let strip = 1280.0 - theme::sidebar_w(1280.0, true);
        let left = theme::HANG + keys + theme::GAP_XL + crate::views::top_bar::ACTS_W;

        // The strip's right side at rest is the gear alone — a
        // `TRANSPORT_HIT` square, not a word with a reserved width (doc 10
        // §7 step 1) — over the window's own gutter.
        let right = theme::TRANSPORT_HIT + theme::HANG;

        assert!(
            left + theme::GAP_LG + right <= strip,
            "the strip wants {:.1} px of {strip:.0}: {left:.1} left \
             (keys {keys:.1}) and {right:.1} right",
            left + theme::GAP_LG + right
        );
        // And the five words really are the bulk of the cluster, so this is
        // measuring the row rather than the acts beside it.
        assert!(keys > 200.0 && keys < 420.0, "the key row is {keys:.1} px");
    }

    /// **The well's two figures fit the lane's measure, and its query fits
    /// beside them** — the reason the counts came out of the well when the
    /// well went into the lane.
    ///
    /// In the strip the counts were the placeholder and the match count sat in
    /// a reserved [`crate::views::top_bar::MATCH_W`] 88 slot *inside* the
    /// field. At [`theme::SIDEBAR_MEASURE`] 232 that slot would leave the
    /// query 100 px, so both figures moved onto the readout line under the
    /// field and both now measure against the whole lane. Longer than the lane
    /// clips rather than reflows, so this is the bound that keeps the ordinary
    /// case whole — and the owner-scale line and a library thirty times that
    /// size are both checked, because the figures tick up during a scan.
    #[test]
    fn the_lanes_well_holds_its_readout_at_the_lane_measure() {
        let sans = sans();
        // The readout hangs between the head's word vertical and the field's
        // own trailing padding — the query's lane, exactly.
        let lane = theme::SIDEBAR_MEASURE - theme::SIDEBAR_HEAD_TEXT_X - theme::GAP_MD;
        for line in [
            "1284 albums · 9902 tracks",
            "40000 albums · 512345 tracks",
            "9902 of 40000 albums",
        ] {
            let measured = sans.width(line, theme::SIZE_META);
            assert!(
                measured <= lane,
                "{line:?} measures {measured:.1} px against the lane's {lane:.0}"
            );
        }
        // And the strip's own form, at the one regime it is still drawn in:
        // the counts as the placeholder, past the magnifier's reserved lane
        // and the input's own padding.
        let strip_lane = crate::views::top_bar::WELL_W
            - (theme::GAP_MD + theme::ICON_PX + theme::GAP_SM)
            - theme::GAP_MD;
        let measured = sans.width("1284 albums · 9902 tracks", theme::SIZE_META);
        assert!(
            measured <= strip_lane,
            "the owner-scale counts measure {measured:.1} px against the \
             strip well's {strip_lane:.0}"
        );
    }

    /// **The strip's declared reservations hold their measured words** —
    /// L9's other half (doc 10 §7 step 7). `theme.rs` asserts the budget as
    /// const arithmetic over the declarations in `views::top_bar`; this
    /// measures each cluster's real words, paddings and gaps in the face
    /// that draws them against its declaration, so neither a font change nor
    /// a relabel can quietly overflow the budget the law adds up.
    #[test]
    fn the_strips_declared_tenant_widths_hold_their_measured_words() {
        use baz_core::index::GroupKey;

        let medium = Face::parse(SANS_MEDIUM);

        // The group-key row: five tracked caps words, `GAP_XS` padding each
        // side, `GAP_MD` between.
        let keys: f32 = GroupKey::ALL
            .iter()
            .map(|key| {
                medium.width(
                    &theme::tracked(&key.label().to_uppercase()),
                    theme::SIZE_META,
                ) + 2.0 * theme::GAP_XS
            })
            .sum::<f32>()
            + 4.0 * theme::GAP_MD;
        assert!(
            keys <= crate::views::top_bar::KEYS_W,
            "the key row measures {keys:.2} px against a declared {}",
            crate::views::top_bar::KEYS_W
        );

        // The acts cluster: the triangle and its word, then two words, each
        // in `GAP_SM` padding, with `GAP_XS` between the three.
        let word = |label: &str| medium.width(label, theme::SIZE_META);
        let acts = (2.0 * theme::GAP_SM + theme::ICON_PX + theme::GAP_SM + word("Play all"))
            + (2.0 * theme::GAP_SM + word("Shuffle"))
            + (2.0 * theme::GAP_SM + word("Pull"))
            + 2.0 * theme::GAP_XS;
        assert!(
            acts <= crate::views::top_bar::ACTS_W,
            "the acts cluster measures {acts:.2} px against a declared {}",
            crate::views::top_bar::ACTS_W
        );

        // The `Playlists` door was measured here until ADR-0030 §5 removed
        // it: the returns lane is the resident index of lists, so the strip
        // has no door to that index any more and no word to measure. What it
        // freed is spent in `theme`'s own budget arithmetic
        // (`the_strip_holds_its_tenants_at_the_single_line_floor`), which is
        // where a *width* claim belongs.
    }

    /// **The returns lane holds its head's three words** at the measure the
    /// open lane gives them.
    ///
    /// The head is the one part of the lane whose copy is fixed and known —
    /// `Home`, `Library`, `Now playing` — so unlike a record's title it can
    /// be *held* rather than clipped. `Now playing` is the long one, and the
    /// glyph, its gap and the row's own padding all come off the measure
    /// before the word gets it.
    #[test]
    fn the_returns_lane_holds_its_three_destinations() {
        let medium = Face::parse(SANS_MEDIUM);
        // What a destination row leaves the word: the lane's content measure,
        // less the row's two `GAP_SM` flanks, the glyph's box and the
        // `GAP_MD` between it and the word.
        let measure =
            theme::MENU_W - 2.0 * theme::GAP_SM - theme::SIDEBAR_GLYPH_BOX - theme::GAP_MD;
        for label in ["Home", "Library", "Now playing"] {
            let word = medium.width(label, theme::SIZE_BODY);
            assert!(
                word <= measure,
                "the lane's {label:?} measures {word:.2} px against {measure:.2} \
                 of room — the head's words are fixed copy and must not clip"
            );
        }
    }

    /// **The index rail's lane holds the labels the keys actually produce** —
    /// or, where it cannot, the test says so by name rather than the wall
    /// discovering it.
    ///
    /// [`theme::INDEX_W`] is **60 px** — ADR-0017 §1.7 as the composition audit
    /// amends it — and the labels are `baz-core`'s own. What the widening bought
    /// is measured here rather than asserted:
    ///
    /// - **Every label the five keys can *produce* fits.** Letters, `#`,
    ///   `Various`, `No year`, every decade, `Unknown` (42.4 px, which is why 36
    ///   was wrong), and every recency bucket down to `Never played`. At 36 the
    ///   rail worked for one of the five arrangements and clipped in three.
    /// - **Arbitrary genre names still elide**, and they always will: a genre
    ///   tag is free text. They clip at the lane's right edge with their heads
    ///   intact, and the full value is set in the shelf header one `HANG` to the
    ///   left at the same moment, in the same voice — the rail is a ruler, not a
    ///   legend.
    ///
    /// The lane cannot simply grow without limit: it comes out of the wall, and
    /// the scrollbar's own lane bounds it from the other side
    /// ([`theme::INDEX_CLEARANCE`]). What this test does is keep the trade
    /// *measured* rather than discovered on a screenshot.
    #[test]
    fn the_index_rail_holds_the_labels_its_keys_produce() {
        let sans = sans();
        for label in [
            // ARTIST
            "#",
            "A",
            "M",
            "W",
            "Ø",
            "曲",
            "Various",
            "Unknown",
            // YEAR
            "No year",
            "1890s",
            "1980s",
            "2020s",
            // ADDED / PLAYED
            "Today",
            "This evening",
            "Never played",
            "Not recorded",
        ] {
            let width = sans.width(label, theme::SIZE_HEADING);
            assert!(
                width <= theme::INDEX_W,
                "{label:?} measures {width:.2} px in a {} px rail",
                theme::INDEX_W
            );
        }
        // The tightest of them, named, because a lane sized to its widest label
        // is a lane whose margin is worth stating: `Never played` measures
        // 59.14 px and the lane is 60. That is the number that decided 60 rather
        // than 56, and if the face or the bucket vocabulary ever moves it, this
        // is where it is caught.
        let tightest = sans.width("Never played", theme::SIZE_HEADING);
        assert!(
            (59.0..60.0).contains(&tightest),
            "the rail's widest produced label now measures {tightest:.2} px"
        );
        // The stated exception, and the only one: a genre is whatever a tagger
        // wrote, so there is no width at which the lane holds all of them. It
        // clips, and the header carries the value.
        let long_genre = "Progressive Electronic";
        assert!(
            sans.width(long_genre, theme::SIZE_HEADING) > theme::INDEX_W,
            "{long_genre:?} now fits the rail — the doc comment saying an \
             arbitrary genre may not is stale"
        );
        // The fisheye's swollen letter still fits the ink's own lane: the
        // widest glyph the ARTIST key draws, at the lens's peak, keeps inside
        // INDEX_W — so a magnified *letter* never even needs the clearance the
        // swell is allowed to borrow, let alone clips. (`W` is the widest; the
        // margin is wide enough that the medium face's slightly broader letter
        // changes nothing.)
        let widest = ('A'..='Z')
            .map(|letter| {
                sans.width(
                    &letter.to_string(),
                    theme::SIZE_HEADING * theme::MAGNIFY_MAX,
                )
            })
            .fold(0.0_f32, f32::max);
        assert!(
            widest < theme::INDEX_W / 2.0,
            "a magnified letter measures {widest:.2} px in a {} px lane",
            theme::INDEX_W
        );
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

        // **The needle's hover tip**, which carries two different kinds of
        // string because a click on the needle means two different things: a
        // timestamp inside the sounding entry, the entry's own name outside it.
        // The timestamp has a bound and is asserted; a title does not — it is
        // free text, and it elides, exactly as a genre name does in the index
        // rail (ADR-0017 §1.7's amendment made the same call). What is asserted
        // for the title is that the slot holds a *useful* one rather than a
        // word and a half.
        fits(
            &sans,
            "0:00:00",
            theme::SIZE_CAPTION,
            theme::NEEDLE_TIP_W,
            "NEEDLE_TIP_W",
        );
        fits(
            &sans,
            "Everything You Do Is a Balloon",
            theme::SIZE_CAPTION,
            theme::NEEDLE_TIP_W,
            "NEEDLE_TIP_W",
        );

        // A setting's value slot: `replaygain::format_centidb` at either end
        // of the pre-amp travel, in the glyphs it actually emits — the minus is
        // U+2212, which advances as wide as the `+` and so keeps the slot's
        // left edge still as the value steps through zero.
        for value in ["\u{2212}20.00 dB", "+20.00 dB"] {
            fits(
                &sans,
                value,
                theme::SIZE_META,
                theme::SETTING_VALUE_W,
                "SETTING_VALUE_W",
            );
        }
        assert!(
            (sans.width("\u{2212}20.00 dB", theme::SIZE_META)
                - sans.width("+20.00 dB", theme::SIZE_META))
            .abs()
                < f32::EPSILON,
            "the two ends of the pre-amp travel must measure the same, or the \
             slot's left edge moves as the value crosses zero"
        );

        // The track/queue number column: three figures, per its own docs.
        fits(
            &sans,
            "999",
            theme::SIZE_META,
            theme::TRACK_NO_W,
            "TRACK_NO_W",
        );

        // The bar's Queue readout. It draws the queue's *size* now, bounded at
        // three figures (`999`) — the position it used to draw moved into the
        // ambient continuation line, which is not a fixed slot and clips. The
        // slot's width is unchanged, so the two strings it was derived for are
        // measured too: `199 / 240` is the spec's worst case and `999 / 999` is
        // the widest the same shape can be, and with tabular figures they are
        // the same width — which is the whole reason the bound can be stated in
        // figures rather than in pixels.
        for position in ["999", "199 / 240", "999 / 999"] {
            fits(
                &sans,
                position,
                theme::SIZE_META,
                theme::POSITION_W,
                "POSITION_W",
            );
        }

        // The bar's Queue control: its label in the Medium face it is set
        // in, inside what the slot has left after the readout and the padding.
        fits(
            &Face::parse(SANS_MEDIUM),
            "Queue",
            theme::SIZE_META,
            theme::UP_NEXT_W - theme::POSITION_W - 3.0 * theme::GAP_SM,
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

        // The strip's Settings door is the gear now (doc 10 §7 step 1): a
        // glyph in a fixed square has no word to measure, and its name rides
        // the tooltip, whose card sizes to its own text.

        // The well's reserved match-count slot (doc 10 §4.1): `7 / 1284` up
        // to a library far larger than the owner's, in the readout's own
        // face and size.
        for count in ["7 / 1284", "40000 / 40000"] {
            fits(
                &sans,
                count,
                theme::SIZE_META,
                crate::views::top_bar::MATCH_W,
                "MATCH_W",
            );
        }
    }

    /// The Settings place's reserved note slot still holds every sentence it
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
        // The width a wrapped line actually has, at the **narrowest** the
        // place is ever set: the floor of `views::settings`'s clamp, less the
        // scrollbar lane the list keeps clear. It read `PANEL_W − 2 × GAP_XL`
        // when the settings were a panel in the rail and the floor was the
        // inspector's content lane; ADR-0022 deleted the column and
        // `SETTINGS_CONTENT_MIN` states the same number directly.
        let content_w = theme::SETTINGS_CONTENT_MIN - theme::SCROLLBAR_LANE;
        let lines = theme::SETTING_NOTE_H / (theme::SIZE_META * theme::LEADING_META);
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
