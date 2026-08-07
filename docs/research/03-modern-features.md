# Modern Features: Local ML, Enrichment, Presentation

> Research groundwork for baz, 2026-08-07. What a new local-library player can offer that foobar2000 and its clones lack — with feasibility flags.

## 1. Local audio analysis / ML — proven and feasible

**Essentia (+ essentia-tensorflow)** is the de facto standard for local music ML. MTG ships pre-trained models: Discogs-EffNet embeddings feeding a genre_discogs400 head (400 Discogs styles, trained on 2M+ recordings) plus mood heads (happy/sad/aggressive/relaxed, danceability, etc.), and the newer transformer MAEST. Real projects run this over user libraries and write results into tags (beets-xtractor, beets-autogenre, dj-tagger). Lightweight models run real-time on laptop CPUs; a full library pass is an hours-scale one-time batch job. Caveat: the Python/TF packaging is awkward to embed in a native player; **ONNX-converted models are the pragmatic route** (what AudioMuse-AI does).

**bliss-rs** is the best "small and native" option: a Rust song-analysis library (timbre/tempo/chroma features + distance metric) built exactly for playlist generation. Measured: ~56 min to decode+analyze 10,000 files; 100k tracks ≈ 9–10 h single-machine, one-time, incremental thereafter — clearly feasible. Proven in the wild: blissify (MPD smart playlists, including "path" playlists that walk song-to-song) and bliss-analyser for Lyrion/LMS. Bliss gives similarity but not semantic labels ("mellow"); pair it with Essentia mood heads or CLAP.

**CLAP embeddings** are the genuinely new capability: joint text–audio space, so "play something mellow" becomes a text query against a local vector index (FAISS/sqlite-vec). LAION-CLAP is ~193M params; runs on CPU but slowly at library scale. **AudioMuse-AI-DCLAP** distilled the audio tower to ~7M params with 5–6× faster inference — evidence that text-to-music search over a personal library is practical today. AudioMuse-AI itself (self-hosted; integrates Jellyfin/Navidrome/LMS/Emby/Plex; sonic-similarity playlists; vector search; no external services) is the closest existing open-source proof of the whole pipeline.

Verdict: **bliss-rs similarity = proven; Essentia mood/genre tagging = proven; CLAP free-text queueing = feasible-but-frontier** (best current bet: distilled CLAP, or Essentia mood scores mapped to a controlled vocabulary).

**AcousticBrainz is dead** (shut down Feb 2022). Don't build on it. Lesson: do analysis locally, not via a community DB.

## 2. Smart shuffle / vibe queueing in the wild

- **Plexamp**: server-side neural "sonic analysis" (~50 weighted parameters) powers track radio, sonically-similar browsing, mood mixes. CPU-intensive, Plex-Pass-gated, closed. The UX benchmark for what baz wants.
- **Roon Valence**: cloud ML combining deep metadata + user behavior; not replicable offline, but its "rich context + recommendations" framing matches baz's now-playing goals.
- **ListenBrainz LB Radio**: open, prompt-based radio ("artist:(radiohead)::nosim tag:(mellow)" style prompts) via the **troi** engine; crucially troi + listenbrainz-content-resolver support **fully local playlists**, resolving MBID playlists against your files (requires MusicBrainz-tagged library). A free, already-built "steered shuffle" backend worth integrating.
- **MPD ecosystem**: blissify, ashuffle — proven but crude UX.
- **Music Assistant** is actively discussing AudioMuse-AI integration — a signal that local-first similarity is where hobbyist infra is heading.

## 3. Metadata enrichment for context panes

- **MusicBrainz**: free, no key; descriptive User-Agent + ~1 req/s. **MBIDs are the linchpin** — they key everything else (Cover Art Archive, Wikidata links, fanart.tv, ListenBrainz). Artist/release-group relationships include URLs to Wikipedia/Wikidata, Discogs, Bandcamp, official sites.
- **Wikipedia/Wikidata**: MusicBrainz links to Wikidata, which bridges to Wikipedia in all languages; fetch article extracts via the Wikipedia REST summary API. Free, CC-licensed, ideal for album backstory/artist bios.
- **Last.fm**: API key required; artist bios, similar artists, user tags, album info. ToS restricts caching; fine for a client app, off by default.
- **TheAudioDB**: artist bios in ~6 languages, mood/style fields, images. **fanart.tv**: high-quality art keyed by MBID. Both are what Kodi's Artist Slideshow uses — a proven pattern for a rich now-playing pane.
- **Discogs**: free API, 60 req/min; strong for credits, pressings, label context.
- **ListenBrainz**: open scrobbling + stats + similar-artist/recording endpoints.
- **Genius**: API returns metadata/annotations but **not lyrics**; scraping violates ToS — avoid or leave to plugins. **LRCLIB** has a free open API for synced lyrics. **Bandcamp**: public API closed; treat as link-out, not data source.
- Reviews/interviews (Pitchfork etc.): no APIs; link-out only — speculative/risky, keep out of core.

Architecture implication: **MBID-tagged library (encourage Picard) + local cache + all network fetches opt-in** matches both the "configurable/off-by-default" wish and every provider's ToS.

## 4. Presentation

- **Album-first, no-playlist UX has a proven existence proof: Longplay** (iOS, MacStories-acclaimed) — a wall/shelf of album art, tap-to-play whole albums, "Album Shuffle" that endlessly queues random full albums. Functionally the GOG-Galaxy-shelf-for-music; **nothing equivalent exists on desktop/Linux.**
- Current lookers in the space: Feishin, Supersonic, Tauon, Cider — all grid-based, none skeuomorphic-shelf. Differentiation opportunity is real.
- **Dynamic color from album art**: proven, cheap. k-means / Vibrant-style palette extraction; Material Design 3 documents deriving full tonal palettes from one source color. Spotify/Apple Music both do this; table stakes for "beautiful."
- **projectM**: actively maintained, cross-platform, Milkdrop-compatible C++ library with built-in beat detection/FFT; renders to an OpenGL context **or a texture** (easy to composite into a modern UI). Visualizers are low-risk, high-delight.

## 5. Other modern capabilities local players lack

- **Scrobbling**: native ListenBrainz + Last.fm dual scrobbling is easy and expected; ListenBrainz feeds its recommendation endpoints back to you.
- **Cross-device play-state sync**: essentially unsolved for pure-local players; requires a companion server or sync service — medium/speculative scope.
- **Remote control**: MPRIS (Linux) is table stakes; phone-as-remote is realistically done by embedding a small HTTP server or speaking OpenSubsonic so existing mobile clients work. Native CarPlay/Android Auto without a mobile app: impractical — speculative.
- **Playback quality**: gapless is mandatory; bit-perfect/exclusive output (WASAPI exclusive/ASIO, CoreAudio hog mode, ALSA direct) is what the audiophile crowd expects. FLAC + Opus dominate; DSD/Atmos niche.
- Modern polish foobar clones lack: waveform seekbars, synced lyrics (LRCLIB), EBU R128 loudness, Discord rich presence.

## Bottom line

The feasible, differentiated core = **bliss-rs similarity + Essentia/ONNX mood-genre tags (batch, local) driving album-first shuffle and "mellow" steering** (controlled vocabulary now, distilled-CLAP free text later); **MBID-keyed enrichment pane** (Wikipedia/Wikidata + TheAudioDB + fanart.tv + Last.fm, opt-in, cached); **Longplay-style shelf + palette-driven theming + projectM**. Cross-device sync and CarPlay are the only wishlist items without a proven local-first path.

## Sources

- https://essentia.upf.edu/models/classification-heads/genre_discogs400/
- https://essentia.upf.edu/tutorial_tensorflow_auto-tagging_classification_embeddings.html
- https://github.com/Polochon-street/bliss-rs ; https://github.com/Polochon-street/blissify-rs ; https://lelele.io/bliss.html
- https://github.com/CDrummond/bliss-analyser
- https://github.com/NeptuneHub/AudioMuse-AI ; https://github.com/NeptuneHub/AudioMuse-AI-DCLAP
- https://www.navidrome.org/docs/usage/integration/audiomuse/
- https://blog.metabrainz.org/2022/02/16/acousticbrainz-making-a-hard-decision-to-end-the-project/
- https://support.plex.tv/articles/sonic-analysis-music/
- https://roon.app/en/valence
- https://troi.readthedocs.io/en/latest/lb_radio.html ; https://github.com/metabrainz/listenbrainz-content-resolver
- https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting ; https://musicbrainz.org/doc/Link_Wikipedia_And_MusicBrainz
- https://api.fanart.tv/ ; https://kodi.wiki/view/Add-on:Artist_Slideshow
- https://www.macstories.net/reviews/longplay-2-0-an-album-oriented-apple-music-player-with-loads-of-new-features/
- https://github.com/projectM-visualizer/projectm
- https://m3.material.io/styles/color/system/how-the-system-works
- https://foxxmd.github.io/multi-scrobbler/
- https://huggingface.co/Xenova/larger_clap_music_and_speech
