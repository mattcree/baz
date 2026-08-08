# baz — Foundations

## Thesis
"A record archive after closing time — the works are lit, the room is not." The collection IS the interface. Audience: people who own their music (collectors, Bandcamp buyers, rippers) arriving from foobar2000/MusicBee. Defensible position: prior art gives the collection 0-26% of the window at rest; baz gives it 73-100%. Never trade this away. Failure mode to guard against: "lobby" — gorgeous at rest, useless in motion. The wall must be a working surface at 40 or 20,000 albums.

The shelf contains exactly two kinds of thing: **artwork and type**. One licensed exception: tiles for user-made objects (mixtapes, deferred) are set in type on a panel surface — never fake art.

## The four rooms (board: 8a, 8b)
A theme is a room, not a skin. One axis: room lightness (warm-neutral throughout). Ink and accent oppose the room. Default = follow system (OS dark -> Closing Time, OS light -> Reading Room); Stone and Plaster are manual picks. Never a room at oklch L .45-.58 (dead zone: neither ink works, mid-value sleeves melt).

| Room | Wall | Panel +1 | Float +2 | Recess -1 | Ink | Accent |
|---|---|---|---|---|---|---|
| Closing Time (L .17) | #0C0D0E | #181716 | #242120 | #070809 | #E8E4DB | amber, glow allowed |
| Stone (L .38) | #4A463F | #55504A | #605B54 | #3F3B35 | #EDE9E0 | amber, ring only |
| Plaster (L .72) | #B7B0A4 | #AAA296 | #9D9589 | #C2BBAE | #211F1C | oxblood |
| Reading Room (L .92) | #E9E4D9 | #DFD9CC | #D5CFC2 | #F2EEE5 | #17161A | oxblood |

Accents: amber = oklch(0.74 0.13 75), oxblood = oklch(0.50 0.14 35). Fixed per room. (Art-derived hue = labelled experiment only; see build guide.)

## Elevation (board: 1a)
- Four levels ever: recess -1, wall 0, panel +1, float +2. Needing +3 means redesign.
- Each step >= 0.03 oklch L. WCAG ratios are meaningless at these lightnesses; do not use them here.
- Surfaces rise toward the lamp: lighter+warmer in dark rooms, darker in light rooms; recesses invert.
- Hairline edges separate (ink at 8-14%, on the lit side); surface deltas group. No shadows, anywhere.
- Recess holds wells only: the groove, input-like regions.

## The accent law
Accent states what is true about playback right now — playing album's halo, playing-track dot, needle fill — nothing else. Never an opaque fill, button color, or decoration. Glow only in Closing Time; elsewhere a 2px ring. Other states: hover = ink 6% overlay; selected = ink ring 55%.

## Type
- IBM Plex Sans only (tabular digits by default — columns self-align). No monospace, no second family.
- Scale: 9-10px caps, 0.14em tracking = shelf headers / group keys / labels (the only chrome voice); 11px working UI; 13px names; poster sizes (40px+) only in Marquee and the filter query.
- Ink opacity is the hierarchy: 100% names, 65% working text, 40-45% metadata/labels, 35% disabled.

## Skeuomorphism rule (board: 9a, 9b)
The record supplies physics, structure, vocabulary — the stack, sides, groove spacing, "drop the needle" — never surface. Banned: vinyl discs peeking from sleeves, wood grain, tonearms, VU meters, wear/patina, any circle pretending to be a record.

## The refusals ledger
Considered and rejected on principle; re-opening one requires beating the argument:
- No autoplay, no radio. Stack empties -> silence. Silence is a feature.
- No invisible shuffle pools — shuffle draws only from what the wall shows.
- No auto-generated playlists; every crate/mixtape is made by a person.
- No engagement stats (no Wrapped, streaks, charts). History records; it never performs.
- No user-picked accent color.
- No view-options menus — group-key words + lens switcher are the entire surface.
- No captions at rest on the wall.
- No mid-gray rooms, no borders on artwork, no radii, no shadows, no motion (hard cuts by design).
