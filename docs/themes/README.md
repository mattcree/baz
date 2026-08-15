# Baz custom themes

Baz themes are local, versioned JSON data. They can change the four room
surfaces, four ink levels, playback mark, status marks, shadow and focus-ring
opacity. They cannot contain code, URLs, paths, fonts, layout or behaviour.
Unknown fields are rejected.

Open **Settings → Appearance** to select one of the six built-ins — Closing
Time, Blue Hour, Stone, Sea Glass, Plaster, Reading Room — paste or import a
JSON document, load an editable template, or export the selected theme. A
successful import is normalized into Baz's config `themes` directory.

**A room you pick stands the moment you press it**, and so does one you
import: the whole application changes on the next frame, not on the next
launch. It used to be the other way round because the glyph sprite sheets were
rasterized once per process in the room's ink; they are now kept per room, and
anything else that bakes a colour — the jewel case's generated textures — is
keyed on which room is standing.

A missing or invalid selected custom theme never prevents startup: Baz reports
the exact error and uses Closing Time. A room that cannot be resolved when you
press it leaves the one you are in standing, and says so.

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
