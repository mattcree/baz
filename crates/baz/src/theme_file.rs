//! Versioned, data-only custom themes and stable built-in selection codes.
//!
//! A theme document contains colours and one bounded focus opacity. It cannot
//! name code, a URL, a path, a font, spacing or behaviour. Unknown fields are
//! rejected so a typo is diagnosed instead of silently becoming a different
//! room. Valid documents live in the application's config `themes` directory.

use std::path::{Path, PathBuf};

use iced::Color;
use serde::{Deserialize, Serialize};

use crate::theme::{self, Palette, Room};

pub const DEFAULT_SELECTION: &str = "closing-time";
pub const BUILTINS: [(&str, &str); 6] = [
    ("closing-time", "Closing Time"),
    ("blue-hour", "Blue Hour"),
    ("stone", "Stone"),
    ("sea-glass", "Sea Glass"),
    ("plaster", "Plaster"),
    ("reading-room", "Reading Room"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub colors: Colors,
    pub focus_opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub recess: String,
    pub wall: String,
    pub plinth: String,
    pub plinth_lit: String,
    pub ink: String,
    pub ink_dim: String,
    pub ink_faint: String,
    pub ink_muted: String,
    pub playback: String,
    pub playback_bright: String,
    pub playback_deep: String,
    pub playback_ink: String,
    pub alert: String,
    pub warning: String,
    pub success: String,
    pub shadow: String,
}

#[derive(Debug, Clone)]
pub struct Preview {
    pub name: String,
    pub colors: [Color; 6],
}

pub fn themes_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("baz").join("themes"))
}

fn custom_id(selection: &str) -> Option<&str> {
    selection.strip_prefix("custom:")
}

pub fn resolve(selection: &str) -> Result<&'static Palette, String> {
    match selection {
        "closing-time" => Ok(&theme::CLOSING_TIME),
        "stone" => Ok(&theme::STONE),
        "plaster" => Ok(&theme::PLASTER),
        "reading-room" => Ok(&theme::READING_ROOM),
        "blue-hour" => Ok(&theme::BLUE_HOUR),
        "sea-glass" => Ok(&theme::SEA_GLASS),
        other => {
            let id = custom_id(other).ok_or_else(|| format!("unknown selected theme {other:?}"))?;
            let dir = themes_dir().ok_or_else(|| "no config directory is available".to_owned())?;
            let path = dir.join(format!("{id}.json"));
            let text = std::fs::read_to_string(&path).map_err(|error| {
                format!("could not read selected theme {}: {error}", path.display())
            })?;
            let (doc, mut palette) = parse(&text)?;
            if doc.id != id {
                return Err(format!(
                    "selected theme id {id:?} does not match document id {:?}",
                    doc.id
                ));
            }
            palette.name = Box::leak(doc.name.into_boxed_str());
            Ok(Box::leak(Box::new(palette)))
        }
    }
}

pub fn preview(selection: &str) -> Result<Preview, String> {
    let palette = match selection {
        "closing-time" => theme::CLOSING_TIME,
        "stone" => theme::STONE,
        "plaster" => theme::PLASTER,
        "reading-room" => theme::READING_ROOM,
        "blue-hour" => theme::BLUE_HOUR,
        "sea-glass" => theme::SEA_GLASS,
        other => {
            let id = custom_id(other).ok_or_else(|| format!("unknown selected theme {other:?}"))?;
            let dir = themes_dir().ok_or_else(|| "no config directory is available".to_owned())?;
            let path = dir.join(format!("{id}.json"));
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            let (doc, palette) = parse(&text)?;
            if doc.id != id {
                return Err(format!("document id {:?} does not match {id:?}", doc.id));
            }
            return Ok(Preview {
                name: doc.name,
                colors: [
                    palette.recess,
                    palette.wall,
                    palette.plinth,
                    palette.plinth_lit,
                    palette.paper,
                    palette.lamp,
                ],
            });
        }
    };
    Ok(Preview {
        name: palette.name.to_owned(),
        colors: [
            palette.recess,
            palette.wall,
            palette.plinth,
            palette.plinth_lit,
            palette.paper,
            palette.lamp,
        ],
    })
}

pub fn import(text: &str) -> Result<(String, PathBuf), String> {
    let (doc, _) = parse(text)?;
    let dir = themes_dir().ok_or_else(|| "no config directory is available".to_owned())?;
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    let path = dir.join(format!("{}.json", doc.id));
    let rendered = serde_json::to_string_pretty(&doc)
        .map_err(|error| format!("could not render theme: {error}"))?;
    std::fs::write(&path, format!("{rendered}\n"))
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok((format!("custom:{}", doc.id), path))
}

pub fn export_document(selection: &str) -> Result<String, String> {
    if let Some(id) = custom_id(selection) {
        let dir = themes_dir().ok_or_else(|| "no config directory is available".to_owned())?;
        let path = dir.join(format!("{id}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let (doc, _) = parse(&text)?;
        return serde_json::to_string_pretty(&doc).map_err(|error| error.to_string());
    }
    let palette = resolve(selection)?;
    serde_json::to_string_pretty(&document_from_palette(selection, palette))
        .map_err(|error| error.to_string())
}

pub fn write_export(path: &Path, selection: &str) -> Result<PathBuf, String> {
    let rendered = export_document(selection)?;
    std::fs::write(path, format!("{rendered}\n"))
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok(path.to_owned())
}

pub fn template() -> String {
    serde_json::to_string_pretty(&document_from_palette("my-theme", &theme::CLOSING_TIME))
        .expect("the built-in theme document serializes")
}

fn parse(text: &str) -> Result<(Document, Palette), String> {
    if text.len() > 64 * 1024 {
        return Err("theme document exceeds the 64 KiB limit".to_owned());
    }
    let doc: Document =
        serde_json::from_str(text).map_err(|error| format!("invalid theme JSON: {error}"))?;
    if doc.schema_version != 1 {
        return Err(format!(
            "schema_version must be 1, found {}",
            doc.schema_version
        ));
    }
    validate_id(&doc.id)?;
    if doc.name.trim().is_empty() || doc.name.chars().count() > 64 {
        return Err("name must contain 1–64 characters".to_owned());
    }
    if !(0.35..=0.85).contains(&doc.focus_opacity) {
        return Err("focus_opacity must be between 0.35 and 0.85".to_owned());
    }
    let c = &doc.colors;
    let palette = Palette {
        room: Room::Custom,
        // The listener-facing name is carried by `Document` during validation
        // and previews. Only launch-time `resolve` promotes it to process-
        // lifetime storage for the active palette.
        name: "Custom theme",
        recess: color("colors.recess", &c.recess, false)?,
        wall: color("colors.wall", &c.wall, false)?,
        plinth: color("colors.plinth", &c.plinth, false)?,
        plinth_lit: color("colors.plinth_lit", &c.plinth_lit, false)?,
        paper: color("colors.ink", &c.ink, false)?,
        paper_dim: color("colors.ink_dim", &c.ink_dim, false)?,
        paper_faint: color("colors.ink_faint", &c.ink_faint, false)?,
        paper_muted: color("colors.ink_muted", &c.ink_muted, false)?,
        lamp: color("colors.playback", &c.playback, false)?,
        lamp_bright: color("colors.playback_bright", &c.playback_bright, false)?,
        lamp_deep: color("colors.playback_deep", &c.playback_deep, false)?,
        lamp_ink: color("colors.playback_ink", &c.playback_ink, false)?,
        alert: color("colors.alert", &c.alert, false)?,
        warning: color("colors.warning", &c.warning, false)?,
        success: color("colors.success", &c.success, false)?,
        shadow: color("colors.shadow", &c.shadow, true)?,
        ring_alpha: doc.focus_opacity,
    };
    validate_palette(&palette)?;
    Ok((doc, palette))
}

fn validate_id(id: &str) -> Result<(), String> {
    let valid = !id.is_empty()
        && id.len() <= 48
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-');
    if valid {
        Ok(())
    } else {
        Err("id must be 1–48 lowercase letters, digits or interior hyphens".to_owned())
    }
}

fn color(field: &str, value: &str, alpha: bool) -> Result<Color, String> {
    let expected = if alpha { 9 } else { 7 };
    if value.len() != expected || !value.starts_with('#') {
        return Err(format!(
            "{field} must be {}",
            if alpha { "#RRGGBBAA" } else { "#RRGGBB" }
        ));
    }
    let byte = |at: usize| {
        u8::from_str_radix(&value[at..at + 2], 16)
            .map_err(|_| format!("{field} contains a non-hexadecimal colour"))
    };
    Ok(Color::from_rgba8(
        byte(1)?,
        byte(3)?,
        byte(5)?,
        if alpha {
            f32::from(byte(7)?) / 255.0
        } else {
            1.0
        },
    ))
}

fn validate_palette(p: &Palette) -> Result<(), String> {
    let surfaces = [p.recess, p.wall, p.plinth, p.plinth_lit];
    let direction = oklab_l(p.plinth) - oklab_l(p.wall);
    for (index, pair) in surfaces.windows(2).enumerate() {
        let step = oklab_l(pair[1]) - oklab_l(pair[0]);
        if step.abs() < 0.03 {
            return Err(format!(
                "surface step {} is {:.3}; every step must be at least 0.030 Oklab L",
                index + 1,
                step.abs()
            ));
        }
        if step.signum() != direction.signum() {
            return Err(
                "recess, wall, plinth and plinth_lit must form one monotonic elevation ladder"
                    .to_owned(),
            );
        }
    }
    let wall_l = oklab_l(p.wall);
    if (0.45..0.58).contains(&wall_l) {
        return Err(format!(
            "colors.wall has Oklab L {wall_l:.3}, inside the 0.45–0.58 dead zone"
        ));
    }
    for (name, ink, floor) in [
        ("colors.ink", p.paper, 4.5),
        ("colors.ink_dim", p.paper_dim, 4.5),
        ("colors.ink_faint", p.paper_faint, 4.5),
        ("colors.ink_muted", p.paper_muted, 3.0),
    ] {
        let lowest = surfaces
            .into_iter()
            .map(|surface| contrast(ink, surface))
            .fold(f32::INFINITY, f32::min);
        if lowest < floor {
            return Err(format!(
                "{name} has contrast {lowest:.2}:1; it must clear {floor:.1}:1 on every surface"
            ));
        }
    }
    let playback_ink = contrast(p.lamp_ink, p.lamp);
    if playback_ink < 4.5 {
        return Err(format!(
            "colors.playback_ink has contrast {playback_ink:.2}:1 on colors.playback; it must clear 4.5:1"
        ));
    }
    for (name, mark, floor) in [
        ("colors.alert", p.alert, 4.5),
        ("colors.warning", p.warning, 3.0),
        ("colors.success", p.success, 3.0),
    ] {
        let ratio = contrast(mark, p.wall);
        if ratio < floor {
            return Err(format!(
                "{name} has contrast {ratio:.2}:1 on colors.wall; it must clear {floor:.1}:1"
            ));
        }
    }
    Ok(())
}

fn luminance(color: Color) -> f32 {
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

fn contrast(a: Color, b: Color) -> f32 {
    let (bright, dark) = if luminance(a) >= luminance(b) {
        (luminance(a), luminance(b))
    } else {
        (luminance(b), luminance(a))
    };
    (bright + 0.05) / (dark + 0.05)
}

fn oklab_l(color: Color) -> f32 {
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let red = linear(color.r);
    let green = linear(color.g);
    let blue = linear(color.b);
    let long = 0.412_221_46 * red + 0.536_332_55 * green + 0.051_445_995 * blue;
    let medium = 0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue;
    let short = 0.088_302_46 * red + 0.281_718_85 * green + 0.629_978_7 * blue;
    0.210_454_26 * long.cbrt() + 0.793_617_8 * medium.cbrt() - 0.004_072_047 * short.cbrt()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a clamped, rounded 0..=255 colour channel is intentionally serialized as u8"
)]
fn hex(color: Color, alpha: bool) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    if alpha {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            channel(color.r),
            channel(color.g),
            channel(color.b),
            channel(color.a)
        )
    } else {
        format!(
            "#{:02X}{:02X}{:02X}",
            channel(color.r),
            channel(color.g),
            channel(color.b)
        )
    }
}

fn document_from_palette(id: &str, p: &Palette) -> Document {
    Document {
        schema_version: 1,
        id: id.to_owned(),
        name: p.name.to_owned(),
        colors: Colors {
            recess: hex(p.recess, false),
            wall: hex(p.wall, false),
            plinth: hex(p.plinth, false),
            plinth_lit: hex(p.plinth_lit, false),
            ink: hex(p.paper, false),
            ink_dim: hex(p.paper_dim, false),
            ink_faint: hex(p.paper_faint, false),
            ink_muted: hex(p.paper_muted, false),
            playback: hex(p.lamp, false),
            playback_bright: hex(p.lamp_bright, false),
            playback_deep: hex(p.lamp_deep, false),
            playback_ink: hex(p.lamp_ink, false),
            alert: hex(p.alert, false),
            warning: hex(p.warning, false),
            success: hex(p.success, false),
            shadow: hex(p.shadow, true),
        },
        focus_opacity: p.ring_alpha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_round_trips_through_the_public_document() {
        for (id, _) in BUILTINS {
            let json = export_document(id).expect("built-in exports");
            let (doc, _) = parse(&json).unwrap_or_else(|error| panic!("{id}: {error}"));
            assert_eq!(doc.id, id);
        }
    }

    #[test]
    fn unsafe_and_malformed_documents_name_the_exact_problem() {
        let mut doc = document_from_palette("safe-room", &theme::CLOSING_TIME);
        doc.colors.wall = "url(https://example.test/a)".to_owned();
        assert!(
            parse(&serde_json::to_string(&doc).expect("test document serializes"))
                .expect_err("unsafe colour must fail")
                .contains("colors.wall")
        );
        doc = document_from_palette("safe-room", &theme::CLOSING_TIME);
        doc.focus_opacity = 2.0;
        assert!(
            parse(&serde_json::to_string(&doc).expect("test document serializes"))
                .expect_err("out-of-range opacity must fail")
                .contains("focus_opacity")
        );
    }

    #[test]
    fn unknown_fields_are_rejected_instead_of_ignored() {
        let json = template().replacen('{', "{\"font_url\":\"https://example.test/font\",", 1);
        assert!(
            parse(&json)
                .expect_err("unknown executable-adjacent fields must fail")
                .contains("unknown field")
        );
    }

    #[test]
    fn committed_examples_are_the_documents_the_runtime_accepts() {
        for (id, json) in [
            (
                "closing-time",
                include_str!("../../../docs/themes/examples/closing-time.json"),
            ),
            (
                "stone",
                include_str!("../../../docs/themes/examples/stone.json"),
            ),
            (
                "plaster",
                include_str!("../../../docs/themes/examples/plaster.json"),
            ),
            (
                "reading-room",
                include_str!("../../../docs/themes/examples/reading-room.json"),
            ),
            (
                "blue-hour",
                include_str!("../../../docs/themes/examples/blue-hour.json"),
            ),
            (
                "sea-glass",
                include_str!("../../../docs/themes/examples/sea-glass.json"),
            ),
        ] {
            let (document, _) = parse(json).unwrap_or_else(|error| panic!("{id}: {error}"));
            assert_eq!(document.id, id);
        }
    }
}
