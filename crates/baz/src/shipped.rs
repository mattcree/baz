//! The part of a module a listener actually runs.
//!
//! # Why this exists
//!
//! iced's widget tree cannot be walked, so a claim about what baz *draws* has
//! no return value to compare against and several dozen tests in this crate
//! asserts over module source instead. Every one of them has to answer the
//! same question first — where does the shipped code stop and the test module
//! begin — and every one of them answered it the same wrong way:
//!
//! ```ignore
//! let code = source.split("#[cfg(test)]").next().expect("a head");
//! ```
//!
//! `#[cfg(test)]` is not the test module. It is also every test-only constant,
//! helper and field in the file, and those sit *above* the functions these
//! scans want to read. On 2026-08-24 three `#[cfg(test)]` members were added
//! near the top of `app.rs` for `App::headless` and five tests
//! across two files broke at once — having been reading a truncated file, in
//! at least one case for as long as `top_bar.rs` has had its audit constant.
//!
//! They broke loudly, because they used `.expect()`. The ones written with
//! `.unwrap_or_default()` would have gone quietly green over an empty string
//! instead, which is the failure this module exists to prevent.
//!
//! # What it does
//!
//! Cuts at the test *module* — `#[cfg(test)]` immediately followed by `mod `
//! at the start of a line, which is the one shape a test module has in this
//! crate — and leaves every other `#[cfg(test)]` where it is.

/// The source above the module's test module.
///
/// Returns the whole of `source` when there is no test module, which is the
/// honest answer for a file that has none.
#[cfg(test)]
pub(crate) fn head(source: &str) -> &str {
    // Anchored at a line start so the marker cannot be matched inside a
    // string literal in the middle of a line.
    const MARKER: &str = "\n#[cfg(test)]\nmod ";
    match source.find(MARKER) {
        Some(at) => &source[..=at],
        None => source,
    }
}

/// The module's shipped code with its comment lines removed.
///
/// [`head`] answers "is this before the tests"; this answers "is this code at
/// all". A scan looking for what a view *draws* must not count a doc comment
/// that merely *names* the thing — `icon.rs` discusses a `Glyph::Baz` it might
/// one day put on the sheet, and the glyph-coverage test read that sentence as
/// a draw the moment `head` stopped truncating the file early.
///
/// Line-based, so a trailing `// ...` after code survives and a `/* */` block
/// is not handled. Both are deliberate: this is a filter for prose that
/// occupies whole lines, which is the shape every comment in this crate takes.
#[cfg(test)]
pub(crate) fn code(source: &str) -> String {
    head(source)
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{code, head};

    /// **A glyph named in prose is not a glyph drawn.** The exact sentence in
    /// `icon.rs` that made the coverage test demand a variant nobody renders.
    #[test]
    fn prose_that_names_a_thing_is_not_code_that_uses_it() {
        let source = "\
/// a monochrome `Glyph::Baz` on the sheet, which is a real option
fn draw() {
    paint(Glyph::Gear);
}
";
        let kept = code(source);
        assert!(
            kept.contains("Glyph::Gear"),
            "the real draw was filtered out"
        );
        assert!(
            !kept.contains("Glyph::Baz"),
            "a doc comment counted as a draw"
        );
    }

    /// **A test-only item above the code is not the boundary.**
    ///
    /// The exact shape that broke five tests: a `#[cfg(test)]` constant,
    /// then the function a scan wants, then the real test module. The old
    /// `split("#[cfg(test)]").next()` stopped at the constant and never saw
    /// the function.
    #[test]
    fn a_test_only_item_above_the_code_is_not_the_boundary() {
        let source = "\
#[cfg(test)]
const WELL_W: f32 = 200.0;

fn draws_the_thing() -> u32 {
    7
}

#[cfg(test)]
mod tests {
    fn draws_the_thing() -> u32 {
        0
    }
}
";
        let kept = head(source);
        assert!(
            kept.contains("fn draws_the_thing() -> u32 {\n    7\n}"),
            "the shipped function was cut away"
        );
        assert!(
            kept.contains("const WELL_W"),
            "a test-only constant is still part of the file above the tests"
        );
        assert!(
            !kept.contains("        0"),
            "the test module's own body came back and would satisfy any scan \
             looking for what the tests mention"
        );
    }

    #[test]
    fn a_file_with_no_test_module_is_returned_whole() {
        let source = "fn only() {}\n";
        assert_eq!(head(source), source);
    }

    /// The marker has to start a line: a mention of it inside a string is
    /// text, not a boundary.
    #[test]
    fn the_marker_must_start_a_line() {
        let source = "fn a() { let s = \"#[cfg(test)]\nmod x\"; }\nfn b() {}\n";
        assert!(head(source).contains("fn b()"), "cut at a string literal");
    }
}
