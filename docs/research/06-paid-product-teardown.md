# Paid Product Teardown: Roon, Plexamp, Audirvana, Swinsian, JRiver, HQPlayer

> Research groundwork for baz, 2026-08-07. baz's competitive positioning: a free, open-source alternative to the paid products — everything they do, except what requires a vendor cloud or account. Every feature classified: **A = purely local** (replicable offline with good engineering), **B = local-network** (LAN, no vendor cloud), **C = vendor-cloud-dependent**.

## 1. Roon ($14.99/mo, $149.88/yr, $829.99 lifetime)

| Feature | Bucket | Notes |
|---|---|---|
| Rich editorial metadata graph: bios, reviews, cross-referenced performer/composer/producer credits, "web of relationships" browsing, Focus faceted search | **C** (licensed TiVo/Rovi data) — the UI itself is A | The #1 thing users say they'd miss. Substitute: MusicBrainz + Wikidata + Discogs + file tags gets ~80% of credits/relationships for popular music; classical work/composition linking and *editorial* reviews/bios have no full open substitute. Focus-style faceted browsing over that data is pure engineering (A). |
| RAAT multi-room: bit-perfect, sample-accurate synced zones to 1,000+ certified endpoints | **B** (protocol is LAN-only) | RAAT is proprietary; substitutes: Squeezebox/SlimProto (Music Assistant uses it), Snapcast (sample-synced open multiroom), plus AirPlay/Chromecast/UPnP outputs. The *certification program* (200+ brands guaranteeing "just works") is the genuinely hard, non-replicable part. |
| MUSE DSP: parametric EQ, convolution room correction, headroom, crossfeed, resampling up to DSD512; per-zone DSP; signal-path display | **A** | Fully replicable (CamillaDSP, SoX, ffmpeg ecosystems). Signal-path visualization is beloved UX polish. |
| Roon ARC: library on your phone anywhere, CarPlay/Android Auto, offline downloads | **C** for relay/NAT fallback; **B**/self-host for the rest | ARC mostly uses UPnP/NAT-PMP port-forwarding on the user's own router; Roon's cloud does discovery/auth. Tailscale/WireGuard gives ~90% substitute with less "just works" polish (that polish is the moat). |
| Valence: ML recommendations, Roon Radio, Daily Mixes, "New Releases for You" from aggregate user behavior | **C** (collaborative filtering needs fleet data) | Substitutes: ListenBrainz collaborative recommendations (open, MBID-keyed) + local content-based similarity (Essentia/bliss). Good "radio from a seed track"; loses cross-user taste discovery quality. |
| Tidal/Qobuz/KKBOX blended with local library | **C** (vendor partnerships + accounts) | No open substitute for the blended-catalog experience. baz should explicitly not chase this. |
| Library management: dedupe, versions/grouping, box sets, multi-storage watch, tag+metadata merge | **A** | |
| Lyrics, waveform seek, internet radio directory | A/B | Radio directory replicable via RadioBrowser. |

**Complaints/lock-in:** 2021 price hike (~40%); Roon 2.0's always-on internet requirement for *local file playback* caused a firestorm (Darko.audio: "zero minutes of offline playback guaranteed") — partially walked back after backlash. Online license check at startup. Cost is the top cancellation reason on forums.
**Genuinely hard to replicate:** TiVo metadata license, Valence's fleet-scale data, Roon Ready certification ecosystem.

## 2. Plexamp (client free since 2023; Plex Pass $6.99/mo, lifetime $120–250 after 2025 hikes)

- **Sonic analysis** (server-side neural net, ~50-dim embedding per track): runs **on the user's own server** but is **Plex-Pass-gated** — bucket **A** technically, C commercially. Open substitute: Essentia/MusiCNN or bliss embeddings locally = 80–90% parity. Powers users' favorite features.
- **Mixes/radio family**: Library Radio, Mixes for You, Style/Mood stations, **Sonic Adventure** (path between two tracks), Time Travel Radio, Guest DJ — **A** once you have local embeddings + listening history. Sonic Adventure is high-delight, low-cost to clone (blissify already does path playlists).
- **Sonic Sage** (LLM playlist builder; needs Plex Pass + Tidal + OpenAI key): **C** — substitutable with local/user-supplied LLM key. Low priority.
- **Player polish**: gapless, sweet fades, loudness leveling, parametric EQ presets, visualizers, waveform seek, Winamp-nostalgia UI — **A**, repeatedly cited in reviews as why people love it.
- **Streaming your library to yourself**: home/LAN part **B**; remote access + auth is **C** (plex.tv account mandatory even for local playback).
- **Complaints/lock-in:** "Plexamp without internet is diabolical" (official forum) — no local playback without cloud auth; downloads break offline; **March 2025: prices raised up to 75% and remote streaming of your own server paywalled** — a radicalizing event that open-source positioning can directly exploit.

## 3. Audirvana (Studio ~$119.99/yr subscription; Origin ~$119 one-time)

- Audiophile playback engine: exclusive/bit-perfect output, upsampling incl. DSD, low-level buffer tricks — **A**.
- UPnP/Chromecast output to network DACs/streamers — **B**.
- Library management, metadata editing, smart playlists — **A**.
- Studio-only: Tidal/Qobuz/HRA integration, radio, continuous metadata updates — **C** (the Studio-vs-Origin split *is* the C bucket; **Origin proves local-only is viable as a product**).
- **Complaints:** 2021 forced-subscription pivot angered one-time-license owners (Origin was damage control); account activation failures; **chronic Remote-app flakiness** (Android can't connect, dropouts) — a warning that phone-remote *reliability* is a make-or-break B feature; UPnP regressions after updates. Login required even for Origin.

## 4. Swinsian ($24.95 one-time, macOS)

Almost entirely bucket **A** — proof that people pay for pure local engineering: wide format support, folder watching, regex bulk tag editing, flexible duplicate finder, smart playlists, iTunes import, scrobbling, and above all *performance on huge libraries*. Slow release cadence is its main criticism. **Swinsian is baz's minimum viable competitor: match it, then add the B-bucket features it lacks** (no remote, no network outputs).

## 5. JRiver MC (~$90) & HQPlayer (~$300+), briefly

- **JRiver**: zones with per-zone DSP/output, convolution, ASIO/WASAPI, UPnP/DLNA + AirPlay + Chromecast + Squeezebox targets. Nearly all **A/B** — evidence the whole audiophile feature set works without any vendor cloud. Weakness: dated UX (an opening for baz).
- **HQPlayer**: 70+ upsampling filters, 36 delta-sigma modulators, GPU PCM→DSD. All **A** but extreme-niche DSP research; don't chase beyond SoX-quality resampling and an output pipeline that could feed HQPlayer/CamillaDSP externally.

## Cross-cutting findings

**What users would miss most:** Roon → metadata/credits browsing and multi-room "just works"; Plexamp → sonic mixes/radio and the gorgeous player; Audirvana → sound-quality engine + UPnP; Swinsian → speed and tag tooling.

**Lock-in grievances (baz's positioning ammunition):** internet/account required for local files (Roon 2.0, Plexamp); price hikes (Roon 2021, Plex 2025 +75% + remote-streaming paywall); subscription pivots (Audirvana Studio); server-side gating of analysis that runs on the user's own hardware (Plex Pass).

**Honest hard parts (where paid products keep an edge):**
1. TiVo-grade editorial metadata and classical work/performance modeling — MusicBrainz is ~80% for credits, far less for reviews/bios/classical.
2. Valence-quality recommendations without fleet data — ListenBrainz helps but is sparser.
3. Roon Ready device certification — you can speak open protocols but can't make 200 hardware brands test against you.
4. ARC's zero-config NAT traversal polish.
5. Streaming-service integration (licensing wall — explicitly out of scope).

## Parity hit-list (buckets A/B only, ordered by value ÷ effort)

1. **Flawless core playback**: gapless, bit-perfect exclusive mode, all formats, ReplayGain/loudness leveling, crossfade. Table stakes for every product above.
2. **Fast large-library management**: folder watching, 100k+ track responsiveness, smart playlists, regex bulk tag editor, duplicate finder (Swinsian parity).
3. **Local sonic analysis** (bliss/Essentia embeddings, background job) — the single feature unlocking most Plexamp magic, ungated.
4. **Mixes/radio built on #3**: track radio, mood/style stations, Sonic-Adventure-style A→B paths, daily mixes from listening history. Huge perceived intelligence, purely local.
5. **Phone-as-remote over LAN** (mDNS discovery, rock-solid reconnect) — Audirvana's #1 failure mode; reliability *is* the feature.
6. **Network outputs**: UPnP/DLNA renderers, Chromecast, AirPlay(2), Squeezebox/SlimProto targets.
7. **DSP chain with signal-path display**: parametric EQ, convolution room correction, crossfeed, SoX-grade resampling, per-output settings.
8. **MusicBrainz/Wikidata/Discogs enrichment**: credits, relationships, artist images, clickable performer browsing, Focus-style faceted filtering — the 80% Roon substitute.
9. **Multi-room synced zones** via Snapcast and/or SlimProto with per-zone volume/DSP grouping.
10. **Waveform seek + polished now-playing** (synced lyrics, visual craft) — cheap, disproportionate delight.
11. **Self-hosted remote listening**: first-class WireGuard/Tailscale documentation or embedded tunnel + offline sync to a mobile client — 90% of ARC with zero vendor cloud.
12. **ListenBrainz/Last.fm scrobbling + local listening stats** (feeds #4).
13. **Internet radio via RadioBrowser**.
14. **iTunes/Music, Roon, Plex library importers** (playlists, play counts, ratings) — switching-cost killer.
15. **Optional local-LLM/user-key playlist prompt builder** (Sonic Sage clone) — low effort, high demo value.

## Sources

- https://www.hifi.blog/roon-2-0-review-intelligent-music-management-at-an-audiophile-level/
- https://www.techhive.com/article/1443872/roon-arc-review.html
- https://archimago.blogspot.com/2025/06/roon-in-2025-10-years-on-sound-quality.html
- https://darko.audio/2022/10/roon-2-0-and-internet-connectivity/
- https://community.roonlabs.com/t/roon-2-0-and-internet-connectivity-its-just-like-1-8-now/215464
- https://help.roonlabs.com/portal/en/kb/articles/roon-ready
- https://help.roonlabs.com/portal/en/kb/articles/arc-port-forwarding
- https://roon.app/en/music/data
- https://community.roonlabs.com/t/how-does-roon-acquire-its-credit-metadata/267353
- https://www.avforums.com/threads/roon-is-it-worth-the-money.2468505/
- https://support.plex.tv/articles/sonic-analysis-music/
- https://techcrunch.com/2021/08/12/plexs-new-feature-matches-your-sonically-similar-music-to-make-playlists
- https://forums.plex.tv/t/plexamp-without-internet-is-diabolical/861159
- https://alternativeto.net/news/2025/3/plex-raises-subscription-prices-up-to-75-and-locks-remote-streaming-behind-a-paywall
- https://help.audirvana.com/support/solutions/articles/202000056197-what-is-the-difference-between-audirv%C4%81na-studio-and-audirv%C4%81na-origin-
- https://www.hifi.blog/audirvana-origin-back-to-basics/
- https://community.audirvana.com/t/no-more-upnp/37394
- https://community.audirvana.com/t/problems-with-audirvana-remote-ipad-and-andriod-phone/13257
- https://swinsian.com/
- https://eclecticpassions.net/blog/swinsian-how-i-chose-the-best-macos-music-library-app/
- https://hifiauditions.wordpress.com/2026/01/25/jriver-media-center-in-2026/
- https://audioreview.frieve.com/products/en/signalyst-hqplayer-5/
- https://github.com/snapcast/snapcast
- https://github.com/orgs/music-assistant/discussions/5133
