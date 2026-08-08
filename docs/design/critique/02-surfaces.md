# baz — Surfaces & Workflows

Scope tags: [v1] ships first; [defer] designed but parked. Board references (1b, 5a...) point to mocks in "baz critique.dc.html".

## The friction budget (binding on every feature)
launch -> music resumes < 200ms (wall position + paused track restored) | intent -> sound = 1 click | keystroke -> filtered wall = next frame | new files -> on the wall with no dialog, ever | tag fix inline, playback never blocked. Anything adding a click before sound argues for its life.

## The wall — home [v1] (1b)
- Covers >= 160px at default density; gutters ~14% of cover width, vertical > horizontal. Square, borderless, unclipped.
- No captions at rest. Selected: ink ring 55% + caption (title / artist-year). Playing: accent halo + caption + accent dot. Hover: ink 6% overlay.
- Shelf breaks from the active group key: 9-10px caps headers at ink 40%, sticky in the virtualizer. Album count top-right at ink 40%.
- Group keys [v1]: ARTIST / YEAR / GENRE / ADDED / PLAYED — one row of words, no menus. Genre verbatim from tags (messy tags show, honestly). CRATES joins later [defer].
- Density is zoom, not a setting: 3-4 discrete stops; full-collection overview (11b) is the last stop [defer].

## Find — type anywhere [v1] (4a)
- No search field exists. Any bare keystroke filters the wall next frame; query renders as ~48px display type bottom-left with match count.
- Enter plays first match; Esc clears. All other shortcuts on a modifier layer (Cmd/Ctrl+...), since bare letters are query.
- Index rail (11a) [v1]: 36px type-only rail, a pure projection of the active group key — ARTIST -> A-Z, YEAR -> decades, GENRE -> genre names, ADDED/PLAYED -> recency buckets (TODAY, THIS WEEK, months, NEVER). Re-derives on key change; no state of its own. Click jumps, drag riffles, PgUp/PgDn snap a shelf. Long value sets elide to near-viewport shelves + first/last.

## Playback — the needle [v1] (5a, 9a)
- No transport bar. 2px seek line flush on the window's bottom edge, segmented by the album's real track lengths — 2px gaps at track boundaries, 6px at a side break. Fill = accent; track = recess -1.
- Wall label bottom-left, 11px: "Title — Artist - elapsed" + stack status when queued ("then 2 sleeves - 1h 58m left"). Bottom-right: "Queue - N" (opens the stack).
- Click a groove segment to jump tracks — no prev/next buttons. Keys: space play/pause, left/right seek, up/down or scroll volume, M mute.
- Hovering the playing cover reveals transport glyphs over the art — the only icons in the app. Hard cut; usability-test it.
- Wall at rest = 100% collection; playback costs 2px + one line of type.

## Lenses [v1: Wall + Marquee] (3a, 2a, 2b)
- One library, shared state (selection/sort/filter/playing). Switcher is type: WALL - MARQUEE (- CRATE later); keys 1/2/3.
- Marquee (2a): playing sleeve at half-window, full-bleed, poster type over a vertex-alpha scrim; wall dims to 35%. Default after ~30s idle while playing; any keystroke snaps back. Hosts the pull (6b).
- Crate (2b) [defer]: spine browsing; needs edge-sampled spine colors + rasterised rotated text.
- Lenses never grow settings: no grid-size pickers, list modes, column options.

## Inspector [v1] (9b, 12b)
- 340px right panel on +1, hairline left edge; toggle Cmd+I. Art, title/artist/year, track list.
- SIDE A / SIDE B headers when rip metadata carries sides — data-driven, never faked. Playing track gets the accent dot.
- The card: "PLAYED - N times since YYYY" + column of date stamps ("12 Mar 2026 - side A only"). No charts.
- Inline tag editing, field by field, playback never blocked. Bulk retagging out of scope forever (Picard exists). Format info (FLAC 24/96) lives here, not in playback chrome.

## The stack — the queue [v1] (6a, 13a)
- Click always just plays. Shift+click stacks a sleeve (or a track from the inspector); numeral chip on the cover is the only mark.
- One queue holding whole sleeves and loose songs; popover on +2 from "Queue - N"; drag to reorder; albums listed as albums, never flattened.
- Ephemeral by design: clears when it ends; empty stack = silence. "Pour into a mixtape" is the save path [defer]. No half-saved queue limbo.

## Shuffle & the pull [v1] (10b, 6b)
- One rule: shuffle plays from whatever the wall currently shows (a shelf, filter matches, everything). Pool always visible: non-pool covers dim to 35%; next two draws carry faint ink rings.
- "Vibe shuffle" = a group key or filter, not a feature. Future MOOD key slots in with no new UI.
- The pull (Cmd+R): one sleeve, weighted toward long-unplayed, presented in Marquee with "last played N years ago". Nothing plays until space; Cmd+R re-pulls; Esc returns.

## History [v1] (12a, 12b)
- Append-only ledger in a plain local file — one line per play; user's to grep/back up/burn. Last.fm scrobbling = optional output, never a dependency.
- Surfaces: PLAYED group key (THIS EVENING -> months -> NEVER PLAYED), inspector card stamps, pull weighting. Nothing else.

## First run & import [v1] (4b)
- Point baz at a folder once. Covers land on the wall as read; recess -1 squares mark what's coming; playback works from the first cover. Header: "watching ~/Music - N of M imported".
- No importer dialog or progress modal, ever. Folder watching uses the same surface; new rips appear under ADDED.

## Curation [defer] (13b, 13c)
- Crates: album-level sets, hand-built, shelves under a CRATES group key; plain text lists.
- Mixtapes: track-level, sequenced, SIDE A/B, optional C60/C90 budget ("1:48 left on side A"); serialized as .m3u8 beside the music; wall tile set in type on +1.

## Settings
Not a screen. A small panel over the wall: music folder, appearance (follow system + four rooms), scrobbling, output device. If it grows past one panel, something upstream went wrong.
