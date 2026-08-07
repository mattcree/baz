# Personas & Experience Research

> UX groundwork for baz, 2026-08-07. Who baz serves, derived experience principles, key tensions, and a first-run sketch.

Grounding: the ownership swing is real and measurable — vinyl passed $1bn revenue with buyers concentrated in the 18–34 bracket, CD sales grew 16% in H1 2026, and download stores persist serving "a dedicated, passionate audience that has no intention of giving up ownership." Motivations: streaming fatigue, artist compensation (Bandcamp's 85/15 split vs. Spotify's ~$0.005/stream), and fear of catalogs disappearing when subscriptions lapse. The tooling these people inhabit: foobar2000, MusicBee, Picard + beets, Navidrome/Jellyfin/Plexamp. baz's opening: no single tool combines foobar-class performance, MusicBee-class library management, and modern cross-platform polish.

## 1. Personas

### P1 — Marta, the Collector-Curator ("the librarian")
**Who:** 34, graphic designer, ~40k tracks, mostly Bandcamp FLAC + ripped CDs, meticulously tagged via Picard with a beets pipeline. Active on r/musichoarder.
**Music in her life:** The library *is* a life project. Curating — fixing tags, hunting cover art, filing by label and era — is genuinely pleasurable, not maintenance.
**Frustrations:** No player treats her painstaking metadata as first-class — custom tags (label, catalog number, rip source) are invisible or need scripting; players silently rewrite or corrupt tags; browsing views are utilitarian and don't reward the work.
**Indispensable:** Read-only-by-default tag handling (never touch files unless asked), arbitrary-tag browsing facets, gorgeous album-art-forward views that make 40k tracks feel like a collection, not a spreadsheet. Fast rescans.
**Bounces on:** Any silent file modification, a hardcoded artist/album/genre-only schema, sluggish scans, cloud-account requirements.

### P2 — Devon, the Album Purist ("the ritualist")
**Who:** 26, Gen Z vinyl buyer, buys the record *and* the Bandcamp download. A few hundred albums, lightly tagged.
**Music in his life:** Sits down and plays albums front-to-back, phone away — intentional listening translated to digital. Tracks are not the unit; albums are.
**Frustrations:** Every modern player is track-and-playlist-centric, shuffles by default, buries albums under recommendation clutter; foobar's default UI felt like a debugging tool and he left in ten minutes.
**Indispensable:** An album shelf as the home screen (GOG-Galaxy-style shelf skeuomorphism lands hardest here), one click = album plays from track 1, gapless, big artwork during playback, maybe liner notes/credits. Zero setup: point at a folder, see a shelf.
**Bounces on:** Anything that looks like Spotify (algorithmic rows, "made for you"), configuration before first playback, panels/toolbars aesthetic.

### P3 — Priya, the Ambient Shuffler ("music as weather")
**Who:** 41, remote software manager, ~15k tracks accumulated over 20 years — old iTunes rips, Bandcamp electronica, ambient labels.
**Music in her life:** Background for work and evenings. Wants to press one button and get hours of *appropriate* music from her own library — "her stuff, not an algorithm's."
**Frustrations:** Drifted back to streaming radio purely because whole-library shuffle is dumb: a harsh noise track lands mid-focus-session. Plexamp's mood radio is the one local tool that gets this, but requires a Plex server and account.
**Indispensable:** Mood/energy-steered whole-library shuffle ("more like this," "calmer," exclude-from-shuffle flags), local audio analysis with no cloud dependency, remembers steering choices. Instant resume.
**Bounces on:** Being forced to build playlists first, shuffle that requires tag hygiene she'll never do, heavy setup.

### P4 — Karl, the Audiophile ("the signal chain")
**Who:** 58, retired engineer, HydrogenAudio lurker, external DAC, mixed FLAC/DSD/hi-res library, SACD rips.
**Music in his life:** Critical listening sessions; equipment and playback correctness are part of the hobby.
**Frustrations:** Bit-perfect exclusive-mode output is a plugin scavenger hunt; gapless breaks across sample-rate changes; Linux audio is a second-class citizen everywhere.
**Indispensable:** Bit-perfect exclusive output (WASAPI/CoreAudio/ALSA) as a visible first-class setting, true gapless, wide format support including cue sheets, ReplayGain done correctly, a status readout proving the chain (source rate → output rate, no resampling). Open source is itself a trust signal — he can verify claims.
**Bounces on:** Mandatory DSP in the path, vague "hi-res" marketing, dropped edge formats, subscription anything.

### P5 — Sam, the Self-Hoster ("the sysadmin of the household")
**Who:** 30, r/selfhosted regular, library on a TrueNAS box, runs Navidrome and Jellyfin, listens from three machines.
**Music in his life:** Music is one service in a homelab; the point is sovereignty — one canonical library, accessible everywhere, no third parties.
**Frustrations:** Subsonic clients are web-grade, not native-grade — sluggish with 100k tracks; play counts/ratings fragment across machines; direct SMB/NFS mounts make desktop players choke or rescan endlessly.
**Indispensable:** First-class network-library support: graceful NAS-mount handling (incremental scans, offline tolerance), ideally an OpenSubsonic client mode so playcounts/ratings/playlists converge. Headless-friendly, scriptable, packaged for Linux (Flatpak/Nix).
**Bounces on:** Single-local-disk assumption, opaque proprietary database, Electron-grade resource usage, phone-home telemetry.

## 2. Core Experience Principles

1. **The library is the interface.** Home screen is the collection itself — an album shelf, not a playlist sidebar or recommendation feed. Playlists exist but are optional; queues are transient. (P1, P2.)
2. **One gesture to music.** Click an album → it plays front-to-back, gapless. Press shuffle → intelligent whole-library flow starts. No queue-building ceremony. (P2, P3.)
3. **Instant everything.** Search-as-you-type across 100k+ tracks in milliseconds; sub-second cold start; scans that never block playback. Performance *is* the foobar2000 heritage and the credibility bar. (All; P5 especially.)
4. **Curation as pleasure, not chore.** Tag editing, art fetching, duplicate review are beautiful, batch-capable, undoable flows — but always opt-in. **baz never modifies a file it wasn't told to. Files are the source of truth; the database is a cache.** (P1, P4 trust; P5 backup-ability.)
5. **Presentation that honors the artwork.** Album art at generous scale, GOG-Galaxy-shelf tactility as one reference; the app should make a collection feel *owned* the way a record wall does. Restraint elsewhere — chrome recedes, art leads. (P2, P1.)
6. **Progressive disclosure.** The default surface is Devon-simple; Karl's output-chain panel, Marta's facet builder, and Sam's sync settings live one deliberate layer down. Never make simplicity the tax for power or vice versa — foobar's plain-default/overwhelming-customization split is the cautionary tale.
7. **Sovereignty by default.** Offline-first, no account, no telemetry, open formats for all app data (playlists as m3u8, exportable database). Internet features (MusicBrainz lookup, art, scrobbling) are enrichments, clearly labeled, individually toggleable.

## 3. Key UX Tensions

- **Simplicity vs. power-user flexibility.** Resolution: a strong opinionated default layout (shelf + now-playing + search) with a capability layer — facets, columns, custom tags, output config — revealed contextually, not a panel-editor-first identity. Power features must be discoverable from the simple surface ("right-click an album → everything").
- **Offline-first vs. internet-enriched.** Resolution: everything works with the network cable cut; online lookups are explicit, per-feature opt-ins offered at the moment they're useful (missing-art badge → "fetch from MusicBrainz?"). Never required; never silent.
- **Curation depth vs. zero-effort start.** Resolution: baz must be excellent with messy libraries — folder-structure inference when tags are absent, fuzzy album grouping — while surfacing gentle, dismissible improvement prompts ("142 albums missing art") rather than gating features on hygiene. Priya must get good shuffle without ever tagging; Marta must get infinite depth.
- **Local-machine vs. multi-machine.** Single-disk assumption alienates P5; server-required design alienates P2/P3. Resolution: local-first core with network sources as a first-class, optional source type.

## 4. First-Run Sketch

Launch → a single quiet screen: "Where's your music?" with a folder picker and a drag-target (auto-suggests ~/Music; "Add a network folder" link for Sam). User picks a folder → scanning begins **and the shelf starts filling immediately**, albums popping in with artwork as found — the scan is the show, no progress-bar purgatory. Within seconds the user can click any already-scanned album and it plays, gapless, while scanning continues. Now-playing shows large art + tracklist. Omnipresent search box (type anywhere to search). One toolbar shuffle button: "Play my library." No account, no wizard, no theme chooser. After the first scan, at most one dismissible notice: "12,431 tracks in. Want baz to look online for missing artwork? [Yes / Not now / Never]" — the sole online prompt, establishing the consent pattern. Audiophile and layout settings exist but are never shown unbidden.

**Target: under 60 seconds and exactly two required decisions (folder, first album) from launch to music playing.**

## Sources

- https://www.soundstagesimplifi.com/index.php/feature-articles/298-the-persistence-of-downloads
- https://www.techradar.com/audio/audio-streaming/forget-spotify-im-going-all-in-on-bandcamp-for-music-in-2025-heres-why-you-should-too
- https://firstfloor.substack.com/p/bandcamp-was-supposed-to-be-dead
- https://www.globalbrandsmagazine.com/vinyl-records-genz-revival/
- https://consequence.net/2026/07/the-cd-revival-is-getting-hard-to-ignore/
- https://amworldgroup.com/blog/vinyl-revival
- https://wiki.hydrogenaudio.org/index.php?title=Gapless
- https://picard-docs.musicbrainz.org/
- https://www.blog.brightcoding.dev/2025/09/18/beets-the-ultimate-music-library-manager-and-musicbrainz-tagger
- https://www.alternativeto.net/software/navidrome/
