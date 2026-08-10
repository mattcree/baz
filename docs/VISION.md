# baz — Vision & Groundwork Synthesis

> foo, bar… baz. An open-source, cross-platform music player for people who own their music — inspired by foobar2000, and deliberately not an attempt to be it.
>
> Synthesized 2026-08-07 from the research in `docs/research/`. This is a living document; nothing here is committed until code exists.

## The one-paragraph pitch

baz is a fast, beautiful, open-source (GPL-3 candidate) music player for Windows/Linux/macOS whose home screen is your album collection — point it at a directory and within a minute you're playing an album, gapless, with no playlist ceremony. It is **inspired by** foobar2000 — instant, correct, no commercial agenda, power under the surface — and makes its own choices from there rather than claiming its inheritance: open source, Linux-first, and a default interface that is the collection instead of a configuration surface. baz is not a replacement for foobar2000 and is not trying to be one; the two are different products for a shared kind of listener. Later, the same core can speak to a NAS or a Navidrome server, and steer whole-library shuffle by mood using local audio analysis — no cloud, no account, ever.

## Why it deserves to exist (the gap)

From the competitor survey (`research/02`): **no player today combines all five of** — (1) cross-platform incl. macOS, (2) native-fast with 100k+ tracks, (3) beautiful and album-oriented, (4) deep local-library power, (5) optional remote/NAS library. The market split into two disjoint halves: local-file power players with 2005 UIs and no network story (fooyin, DeaDBeeF, MusicBee, Quod Libet), and beautiful streaming-style clients with no local-file engine (Feishin, Supersonic, Plexamp). The only products bridging it — Roon ($830 lifetime) and Plexamp (Plex Pass, account required) — are closed and increasingly resented for pricing. The audience (Bandcamp buyers, rippers, hoarders, self-hosters — see `research/05`) is growing, not shrinking.

**baz = fooyin's engine discipline + Plexamp/Tauon's design sensibility + Feishin's server reach, in one open native codebase.**

## Competitive target: the paid products

baz's real competition is not other free players — it's **Roon, Plexamp, Audirvana, Swinsian, JRiver** (teardown in `research/06`). The positioning: **a completely free, convenient alternative that does pretty much everything they do, except anything that requires a centralized vendor server or account.** People should not be tempted into subscriptions and lifetime licenses by convenience alone; convenience is precisely what baz must provide for free.

The teardown classifies every paid feature into three buckets:

- **A — purely local** (replicable offline with good engineering): the entire Audirvana/Swinsian value proposition, Roon's DSP engine, Plexamp's player polish *and* its sonic analysis (which already runs on the user's own hardware — Plex merely paywalls it).
- **B — local-network, no vendor cloud**: multi-room zones (Snapcast/SlimProto), phone-as-remote, UPnP/Chromecast/AirPlay outputs, streaming your library to your own devices.
- **C — genuinely vendor-cloud**: Roon's licensed TiVo metadata and Valence fleet recommendations, ARC's relay, Plex account auth, Tidal/Qobuz integration. Out of scope on principle; open substitutes (MusicBrainz/Wikidata, ListenBrainz, WireGuard/Tailscale) recover ~80% where it matters.

**Prioritization stance: parity features are extensions, not the core.** The basic thing — instant local playback, the shelf, the library — comes first and must be excellent on its own. The bucket A/B parity list (the 15-item "hit-list" in `research/06`) is documented as *where we want to get to*, deliberately not what v0.x is judged by. The lock-in grievances the paid products generated in 2021–2025 (internet required for local files, +75% price hikes, remote-streaming paywalls, subscription pivots) are the standing argument for baz's existence.

## What we take from foobar2000 — and what we refuse

Inherit (identity, non-negotiable): instant startup and search; tiny footprint; gapless always; correct ReplayGain; bit-perfect output as a visible setting; mass-capable tagging; keyboard-driven flow; zero nags/telemetry/accounts; scientific honesty (no audiophile snake oil).

Refuse (the fixes): closed source; Windows-first; configuration-before-usability ("the good experience is hours away"); track/playlist-centric default view; component-ecosystem fragility as an excuse for a bare core.

Betrayal list (things that would lose the community, from `research/01`): Electron-grade sluggishness, telemetry/accounts by default, dumbing down that caps the power ceiling, forced streaming integration, skin-first development, snake-oil audio claims.

Both lists have a standing successor: **[the product's standing rules](the product's standing rules)** — things considered and rejected on principle, where an entry leaves only by an ADR that beats its argument. New refusals go there, not here.

## Product pillars

1. **The library is the interface.** Home = album shelf (GOG-Galaxy-style tactility as one reference; iOS Longplay is the proven pattern). One click plays an album front-to-back. Playlists optional, queues transient.
2. **Instant everything.** Sub-second cold start; search-as-you-type over 100k+ tracks in milliseconds (in-RAM index, SQLite behind it); scans never block playback.
3. **Sovereignty by default.** Open source, offline-first, no account, no telemetry. Files are the source of truth; the database is a cache; baz never writes to a file unbidden. All app data in open formats.
4. **Steered shuffle, locally.** "Play my library" with mood/energy steering ("calmer", "more like this") powered by local analysis (bliss-rs similarity + Essentia/ONNX mood tags; distilled-CLAP free-text later). No cloud dependency — this is Plexamp's killer feature without the Plex.
5. **Opt-in enrichment.** MBID-keyed context pane: album backstory (Wikipedia/Wikidata), artist bios (TheAudioDB), art (Cover Art Archive/fanart.tv), scrobbling (ListenBrainz/Last.fm), synced lyrics (LRCLIB). Every fetch explicit, cached, individually toggleable, off by default.
6. **Progressive disclosure.** Devon-simple surface; Karl's output chain, Marta's tag facets, Sam's server settings one deliberate layer down. v1 is a fixed layout with collapsible panels; foobar-style layout flexibility is a later chapter, not the identity.

## Architecture & stack (recommended)

Full analysis in `research/04`. Headline:

- **Rust workspace, headless-core-first.** `baz-core` crate: Symphonia decode → custom gapless ring-buffer engine → cpal shared-mode + thin native exclusive backends (`wasapi` crate / CoreAudio hog mode / ALSA `hw:`); rusqlite + FTS5 persistence with an in-memory search index; `notify` file watching. Core API = serde-serializable commands/events, so GUI-in-process today can become a server transport tomorrow.
- **GUI: iced** — decided empirically by the Phase 1 spike head-to-head (ADR-0005, 2026-08-07): equal search performance, but iced needed zero Linux system deps and stayed FPS-stable under fling-scroll where Tauri/WebKitGTK visibly janked. Costs accepted: hand-rolled widgets, AccessKit-dependent accessibility. The headless core keeps the choice reversible. Electron, BASS, GStreamer, Flutter: rejected (reasons in `research/04`).
- **Remote libraries: OpenSubsonic client mode** (v2+) — one API covers Navidrome/gonic/Ampache and friends. Abstract the track-source layer from day one (local scanner and remote provider behind one interface) even though v1 ships local-only. A `baz-served` OpenSubsonic *server* wrapping the same core is the long-game option.

## Staged scope (sketch, not commitment)

- **v0.1 "it plays"**: scan directory → SQLite + RAM index; shelf view with art; click album → gapless playback; instant search; MPRIS/media keys; fixed layout with hideable panels.
- **v0.2 "it respects"**: ReplayGain (read + scan), cue sheets, watch folders, batch tag editing (undoable, opt-in writes), exclusive-mode outputs with chain readout.
- **v0.3 "it flows"**: bliss-rs analysis pipeline (background, incremental), steered shuffle, album shuffle, exclude-from-shuffle.
- **v0.4 "it knows"**: opt-in enrichment pane (MBID pipeline), scrobbling, synced lyrics, palette-driven theming, projectM visualizer.
- **Later chapters — the paid-parity extensions** (documented destination, not priority; see `research/06` hit-list): OpenSubsonic client (NAS/remote), phone-as-remote over LAN, network outputs (UPnP/Chromecast/AirPlay/SlimProto), multi-room zones (Snapcast), DSP chain with signal-path display, Roon-style credits/faceted browsing on MusicBrainz data, self-hosted remote listening (WireGuard/Tailscale-first), library importers (iTunes/Plex/Roon), free-text mood queries (distilled CLAP), layout engine, plugin API (design early, stabilize late — foobar's ecosystem fragility is the cautionary tale).

## Open questions

- License: GPL-3 (fooyin/Navidrome precedent) vs MPL-2 (friendlier to embedding)?
- Name collision check before first release (crates.io, package repos) — "baz" is short and common as a placeholder.
- Tauri's Linux leg (WebKitGTK) needs an early spike with a 100k-track virtualized shelf before committing; if it disappoints, iced is the escape hatch.
- How far to lean into shelf skeuomorphism vs a flatter art-forward grid — needs visual prototypes.
- Plugin story timing: a stable extension API too early ossifies internals; too late loses contributors.

## The research shelf

- `research/01-foobar2000.md` — what baz took from it, its community's values, the betrayal list
- `research/02-competitors.md` — the landscape, the gap, OpenSubsonic implications
- `research/03-modern-features.md` — local ML, enrichment sources, presentation tech, feasibility flags
- `research/04-tech-stack.md` — language/audio/GUI/index/architecture analysis and recommendation
- `research/05-personas.md` — Marta, Devon, Priya, Karl, Sam; experience principles; first-run sketch
- `research/06-paid-product-teardown.md` — Roon/Plexamp/Audirvana/Swinsian/JRiver feature-by-feature; local/LAN/cloud buckets; parity hit-list
