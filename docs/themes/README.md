# Baz custom themes

Baz themes are local, versioned JSON data. They can change the four room
surfaces, four ink levels, playback mark, status marks, shadow and focus-ring
opacity. They cannot contain code, URLs, paths, fonts, layout or behaviour.
Unknown fields are rejected.

Open **Settings → Appearance** to select one of the four built-ins, paste or
import a JSON document, load an editable template, or export the selected
theme. A successful import is normalized into Baz's config `themes` directory
and selected for the next launch. The preview updates immediately; the whole
application changes after restart because its glyph texture atlas is built
once per process. A missing or invalid selected custom theme never prevents
startup: Baz reports the exact error and uses Closing Time.

Documents use `schema_version: 1`; see [`theme.schema.json`](theme.schema.json)
and [`examples/`](examples). Every surface step must differ by at least 0.030
Oklab L, room surfaces must avoid L 0.45–0.58, readable inks must clear WCAG
4.5:1, locatable marks 3:1, and focus opacity is bounded to 0.35–0.85.

## Prompt for an external AI

> Create a Baz music-player theme as JSON matching the attached v1 schema.
> Return JSON only. Use a lowercase stable id. Make recess, wall, plinth and
> plinth_lit a monotonic elevation ladder with at least 0.030 Oklab-L between
> neighbours; keep every surface outside Oklab-L 0.45–0.58. Ensure ink,
> ink_dim and ink_faint clear WCAG 4.5:1 on every surface, ink_muted and status
> marks clear 3:1, and playback_ink clears 4.5:1 on playback. Use only hex
> colours and focus opacity; do not add URLs, paths, fonts, layout or code.

Paste the returned JSON into Baz. Its runtime validator remains authoritative;
an AI's claim that a colour passes is not trusted.
