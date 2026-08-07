# Competitor Landscape: Local-Library Music Players

> Research groundwork for baz, 2026-08-07. Survey of local-library players and the self-hosted server ecosystem; where the genuine gaps are.

## Player-by-player capsule reviews

**foobar2000 (reference point)** — Windows (mac port limited); C++/Win32; freeware, closed-source core with open SDK. The archetype: modular UI, massive plugin ecosystem, gapless, ReplayGain, mass tagging, converter, bit-perfect output, extreme performance. Complaints: dated look, Windows-only, aging plugin culture.

**fooyin** — Linux + Windows (macOS "coming soon"); C++23/Qt6; GPL-3.0. The most direct open-source foobar2000 clone: layout-editing mode, FooScript title formatting, plugin API, MPRIS, scrobbling, tag/artwork tools, audio conversion. Very active. In 2026 Linux discussions it's the single most recommended foobar-style player. Complaints: near-empty plugin ecosystem vs foobar2000; no macOS yet; foobar-style aesthetics rather than modern polish; no remote-library support.

**DeaDBeeF** — Linux, Windows, macOS builds; C with GTK UI (core is UI-agnostic); zlib/GPL mix. Ultra-lightweight, plays everything including tracker/chiptune formats, cue sheets, plugins. 15+ years old, still maintained. Complaints: no library/database concept — manual playlist management only; utilitarian, dated UI.

**MusicBee** — Windows only; C#/.NET; freeware, closed source. Arguably the best all-round library manager anywhere: auto-tagging, CD ripping, conversion, skins, device sync, huge feature depth. Complaints: steep learning curve, UI clutter, closed source, no Linux/macOS. One-developer project, updates slow but alive.

**Strawberry** — Linux/Windows/macOS; C++/Qt6 + GStreamer; GPL-3. Clementine fork "for audiophiles": bit-perfect WASAPI/ALSA output, format breadth, Subsonic *and* Tidal/Qobuz support, ListenBrainz. Active, essentially one maintainer. Complaints: dense mid-2000s UI, no real album-grid browsing.

**Quod Libet** — Linux/Windows/macOS; Python/GTK + Mutagen; GPL-2. Power-user library queries (regex + boolean logic), best-in-class tag editing (Ex Falso), Python plugins, scales to huge libraries. Mature but maintenance-pace. Complaints: dated GTK UI, no remote library story.

**Tauon Music Box** — Linux (Windows builds, macOS experimental); Python + SDL2/OpenGL custom UI; GPL-3. Unique: a genuinely *designed*, playlist/album-folder-oriented UI with themes, gapless, CUE, scrobbling, and Subsonic/Airsonic + koel network-library support. Praised for being pretty *and* simple. Complaints: Python perf ceiling, quirky custom toolkit, smaller community, slowing development.

**Clementine** — cross-platform; C++/Qt5; GPL-3. Legacy project; its energy moved to Strawberry. Reference only.

**Swinsian** — macOS only; ObjC/Cocoa; paid closed source ($34.95). "iTunes done right" for local files: fast native UI, big-library performance, watch folders, good tagging. Swinsian 3 (Aug 2025) added Apple Silicon native + dark mode. Complaints: mac-only, closed, historically slow cadence.

**Cog** — macOS only; ObjC/Swift; GPL. Minimal, fast, huge decoder breadth (incl. game/tracker formats), low footprint, privacy-clean. Complaints: minimal library management; reported perf issues with very large libraries.

**Audirvana** — macOS/Windows; closed; Studio subscription $69.99/yr or Origin $119.99. Audiophile playback engine, Qobuz/Tidal, UPnP. Loud complaints: buggy, "mediocre UX," subscription resentment.

**Roon** — Core server + remotes; closed; $12.49–14.99/mo or $829.99 lifetime (Harman-owned). The benchmark for rich metadata graph browsing, multi-room (RAAT), DSP. Complaints: price, mandatory internet even for local files, dependency risk. Proof there's demand for beautiful local-library experiences people will pay heavily for.

**Plexamp** — all desktop + mobile, against a Plex server; closed; best features behind Plex Pass (lifetime $249.99 after big 2025 hikes). The strongest UX in the whole space: sonic analysis, mixes, loudness leveling, gorgeous design. Complaints: server + account + subscription lock-in; Plex's 2025 monetization moves eroded trust. **Plexamp is the UX bar baz should measure against.**

## Server side / remote-library ecosystem

- **Navidrome** — Go, GPL-3, very active. De-facto standard self-hosted music server; Raspberry-Pi-class resource use, multi-user, exposes Subsonic + OpenSubsonic API, large client ecosystem.
- **Jellyfin** — C#/.NET, GPL-2; general media server whose music support is serviceable but secondary; clients (Finamp mobile, Feishin desktop) carry the experience.
- **MPD** — C++, GPL-2; venerable local daemon with its own protocol and deep client ecosystem; beloved by power users but headless and LAN-oriented.
- **Subsonic/OpenSubsonic** — original server is dead-ish/proprietary, but its HTTP API became the lingua franca; OpenSubsonic adds backward-compatible optional extensions (API-key auth, synced lyrics, transcoding control, playback reporting), implemented by Navidrome, gonic, Ampache, etc.
- Notable Subsonic desktop clients: **Feishin** (TypeScript/Electron + MPV, ~9k stars, Spotify-like UI) and **Supersonic** (Go/Fyne, lightweight). Neither handles *local files* — they are remote-only, the mirror image of the desktop players above.

## Comparison table

| Player | Platforms | Stack | License | Maintained | Signature strength | Main weakness |
|---|---|---|---|---|---|---|
| fooyin | Linux, Win (mac soon) | C++23/Qt6 | GPL-3 | Very active | foobar-style modular UI + scripting | No remote lib, empty plugin ecosystem, utilitarian look |
| DeaDBeeF | Linux/Win/mac | C/GTK | zlib+GPL | Active | Lightweight, format breadth | No library database |
| MusicBee | Windows | C#/.NET | Freeware closed | Slow-active | Deepest library management | Windows-only, closed |
| Strawberry | Win/mac/Linux | C++/Qt6/GStreamer | GPL-3 | Active (1 dev) | Bit-perfect output + Subsonic | Dated dense UI |
| Quod Libet | Win/mac/Linux | Python/GTK | GPL-2 | Maintenance | Query language, tag editing | Dated UI, no remote |
| Tauon | Linux (Win beta) | Python/SDL2 | GPL-3 | Active-ish | Designed album UI + Subsonic | Python perf, custom toolkit |
| Swinsian | macOS | ObjC/Cocoa | Paid closed | Active | Fast native mac library UX | Mac-only, closed |
| Cog | macOS | ObjC/Swift | GPL | Active | Speed, decoder breadth | Minimal library features |
| Audirvana | mac/Win | Closed | $119 or sub | Active | Audiophile engine | Buggy UX, pricing anger |
| Roon | Core+remotes | Closed | $830 lifetime | Active | Metadata graph, multiroom | Price, internet-required |
| Plexamp | All desktop+mobile | Closed | Plex Pass | Active | Best-in-class UX, sonic analysis | Server+account+sub lock-in |
| Feishin | Win/mac/Linux | TS/Electron+MPV | GPL-3 | Active | Spotify-grade UI for Navidrome/Jellyfin | Remote-only, Electron weight |
| Supersonic | Win/mac/Linux | Go/Fyne | GPL-3 | Active | Light Subsonic client | Remote-only, Fyne UI limits |
| Navidrome | server | Go | GPL-3 | Very active | OpenSubsonic standard-bearer | Server only |
| Jellyfin | server | C#/.NET | GPL-2 | Very active | Free full media server | Music is second-class |
| MPD | daemon | C++ | GPL-2 | Stable | Rock-solid, huge client base | Own protocol, power-user only |

## Synthesis

**The genuine gap.** No player today combines all five of: (1) cross-platform incl. macOS, (2) native-fast (non-Electron, instant with 100k+ tracks), (3) beautiful/album-oriented, (4) deep local-library power (tagging, gapless, ReplayGain, customization), (5) optional remote/NAS library. The market has split into two disjoint halves: local-file power players with 2005 UIs and no network story (fooyin, DeaDBeeF, Quod Libet, MusicBee), and beautiful streaming-style clients with no local-file story (Feishin, Supersonic, Plexamp). The only closed-source products bridging it — Roon and Plexamp — charge heavily and require accounts/servers, and 2025's pricing moves generated visible resentment: a receptive audience for an open alternative.

**Closest existing players and what they'd need to change.** *fooyin* is closest in spirit (fast C++/Qt, modular, active) but would need macOS, a designed album-first default UI, and a Subsonic backend. *Strawberry* has the platform matrix and Subsonic already, but is one-maintainer legacy-fork code with a UI needing a ground-up rethink. *Tauon* proves beautiful + album-oriented + Subsonic is possible in open source, but its Python/SDL stack caps performance and portability. *Feishin* has the right UX but wrong architecture (Electron, no local engine). **baz effectively = fooyin's engine discipline + Tauon/Plexamp's design sensibility + Feishin's server integration, in one native codebase.**

**What OpenSubsonic means for baz.** Implementing an OpenSubsonic *client* is the cheapest possible route to "optional remote/NAS library": one well-documented HTTP API instantly covers Navidrome, gonic, Ampache, Astiga, and more; extensions are optional and backward-compatible. Design implication: **abstract the library/track-source layer from day one** (local scanner vs Subsonic provider behind one interface) so browsing, playlists, and playback treat both identically — that abstraction is exactly what every incumbent local player lacks.

## Sources

- https://lobste.rs/s/bpqtph/state_linux_music_players_2026
- https://github.com/fooyin/fooyin
- https://ubuntuhandbook.org/index.php/2025/06/fooyin-foobar2000-qt-desktop/
- https://deadbeef.sourceforge.io/
- https://en.wikipedia.org/wiki/MusicBee
- https://www.strawberrymusicplayer.org/
- https://quodlibet.readthedocs.io/
- https://delightlylinux.wordpress.com/2025/05/01/tauon-music-box-music-player/
- https://swinsian.com/blog/2025/08/19/swinsian-3/
- https://cog.losno.co/
- https://www.trustpilot.com/review/www.audirvana.com
- https://roon.app/en/pricing
- https://hostbor.com/plex-changes-explained/
- https://www.navidrome.org/docs/overview/ ; https://www.navidrome.org/apps/
- https://opensubsonic.netlify.app/ ; https://opensubsonic.netlify.app/docs/extensions/
- https://dev.co/devops/open-source/feishin
- https://jellywatch.app/blog/jellyfin-music-server-complete-guide-clients-scrobbling-2026
- https://wiki.archlinux.org/title/Music_Player_Daemon ; https://www.musicpd.org/clients/
