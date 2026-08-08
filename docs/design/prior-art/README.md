# Prior-art renders

Evidence for [`../03-interface-prior-art.md`](../03-interface-prior-art.md).
Every image here is **our own capture** of GPL-licensed software, showing **our
own generated cover art**, rendered the way the design documents render baz
itself — and nothing the maintainer had running was touched.

## How they were made

- a **throwaway podman container** from `registry.fedoraproject.org/fedora:42`,
  with the players installed by `dnf`. The container and the image committed
  from it were removed afterwards (`podman rm -f` / `rmi -f`, verified);
- a **private `Xvfb :171`** at 1600×1000 with `openbox`, on a display number
  nothing else was using — two sibling agents were on `:137` and `:77` and
  neither was addressed;
- **scratch everything**: `HOME=/root` inside the container with its own
  `XDG_*`. The maintainer's `~/Music`, library database and config were never
  opened;
- **silence by construction**: no `/dev/snd` was mapped into the container, so
  there was no audio device to open. Belt and braces, the scratch `HOME` carried
  an `.asoundrc` routing ALSA's default PCM to `null`;
- a **throwaway fixture** of 26 albums / 225 tracks of digitally silent FLAC
  with generated covers in five visual idioms, plus one deliberately untagged
  folder. Generated fresh for this work; never `~/Music`.

One peer could not be captured and is recorded as a failure rather than quietly
dropped: **Elisa 25.12.3 would not start on the headless display**, with or
without `QT_QUICK_BACKEND=software`, and produced no diagnostic output. Its
structure in the study is read from its `.qml` sources instead, and labelled as
such.

## The frames

| Image | Product | What it shows |
|---|---|---|
| [`fooyin-01-first-run-layout-editing-mode.png`](fooyin-01-first-run-layout-editing-mode.png) | fooyin 0.9.2 | **The first frame the product ever draws.** A window titled *"fooyin — Layout Editing Mode"* behind a *"Quick Setup"* modal offering six layout presets with **`Empty` preselected**, over a canvas reading *"Right-click to add a new widget"*. The layout question, asked before the music question |
| [`fooyin-02-simple-scanned-playlist-empty.png`](fooyin-02-simple-scanned-playlist-empty.png) | fooyin | The "Simple" preset **after the scan found 26 artists**. The library is a 19% left column; 80% of the window reads *"Playlist empty"*. The tradition's deepest structural choice, in one frame |
| [`fooyin-03-vision-layout.png`](fooyin-03-vision-layout.png) | fooyin | "Vision", the art-forward preset: 51% of the window is the *playing* artwork — a placeholder disc, since nothing is playing — and the library is collapsed to a vertical tab |
| [`fooyin-04-obsidian-layout.png`](fooyin-04-obsidian-layout.png) | fooyin | "Obsidian", the Columns-UI-style power layout: two facet rails (26%), playlist (50%), art + metadata inspector (21.5%), transport at the bottom. A 26-album library rendered as two columns of text |
| [`fooyin-05-ember-layout.png`](fooyin-05-ember-layout.png) | fooyin | "Ember": four facet rails across the top taking ~25% of the height, with the transport bisecting the window horizontally |
| [`fooyin-06-obsidian-album-loaded.png`](fooyin-06-obsidian-album-loaded.png) | fooyin | Double-clicking an album in the facet rail. It spawned a new *"Filter Results"* playlist tab, appended the album **twice**, and **started nothing**. Also the twenty-field Selection Info inspector on the right — bit depth, sample rate, codec, tag types |
| [`strawberry-01-collection-empty.png`](strawberry-01-collection-empty.png) | Strawberry 1.2.18 | The full chrome: an icon rail (4.9%) where **Queue is a first-class destination** beside Collection and Playlists, a collection tree (17%), and a 78% playlist under a watermark logo |
| [`lollypop-01-first-run.png`](lollypop-01-first-run.png) | Lollypop 1.4.45 | Transport in the header bar, top-left; a 13% nav rail leading with *Suggestions*; and the enrichment consent question as a **dismissible banner over the content**, not a modal |

## Licensing

fooyin, Strawberry and Lollypop are GPL-3. These are screenshots of free
software running our own fixture data, made for documentation and commentary,
and are committed rather than linked for that reason. Proprietary products
(Plexamp, Roon, Apple Music, Spotify, Longplay, Doppler, Albums) are **linked
and described in prose** in the study instead; no copyrighted product imagery is
committed to this repository.
