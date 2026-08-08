# baz — Interface prior art

> The study that `01-ux-audit-and-ia.md` and `02-visual-language.md` were written
> without. Both reason from first principles against the personas and the
> vision; both cite Plexamp as "the UX bar" without anyone having looked at how
> it lays out. The owner's question: *"did we do some significant analysis of the
> prior art that's out there in terms of interface? we probably want to be guided
> heavily by that, while trying to keep a modern, sleek, look."* And then the
> sharpening: *"get examples and study them"*, and *"you should describe all
> common workflows based on their user flows — and then make the interface
> support that easily."*
>
> So the spine of this document is **workflows**, read off the interfaces rather
> than invented from our personas, ranked by how often baz's audience performs
> them, with the layout recommendation derived from that ranking. The
> comparative matrix, the queue-placement evidence and the verdict on the
> proposed IA all serve that.
>
> **Every recommendation in §8 is marked *confirms*, *refines* or *contradicts***
> against what is being built right now. Increments 1–5 of the audit's §5 have
> landed (`25e8d4c`); 6–8 — the Up next popover, interactive queue rows, and
> Settings as a place — are in flight as this is written. Two recommendations
> contradict a shipped spec. They are flagged in bold and argued.
>
> **Read §5 first if you are implementing increment 6.** The short version: the
> popover is **supported** — three album-first products independently make the
> queue a child of now-playing rather than a peer of the library — but it needs
> five changes to survive contact with what everyone else learned, and the two
> that matter most are that *transient must not mean unverifiable* (Plexamp's
> hidden queue has generated years of complaints) and that baz has not decided
> **what happens when an album ends** (§5.4), which is the one thing every
> album-first product has had to fix.

---

## 0. Method — what was actually examined

The brief was explicit that reading a review saying "Plexamp is beautiful" is
worth nothing. So this document is built from three tiers of evidence, and each
claim below says which tier it came from.

### 0.1 Peers run and rendered first-hand

Four GPL players were installed into a **throwaway podman container** built from
`registry.fedoraproject.org/fedora:42` and rendered on a **private `Xvfb :171`**
at 1600×1000. The container is gone; so is the image it was committed from
(`podman rm -f` / `rmi -f`, verified). Nothing the maintainer had running was
touched — two sibling agents were on `:137` and `:77`, and this work never
addressed either.

- **Scratch everything.** `HOME=/root` inside the container, its own
  `XDG_*`, its own config. The maintainer's `~/Music`, database and config were
  never opened.
- **Silence by construction.** No `/dev/snd` was mapped into the container, so
  there was no audio device to open at all; belt and braces, the scratch `HOME`
  carried an `.asoundrc` routing ALSA's default PCM to `null`.
- **A throwaway fixture**: 26 albums / 225 tracks of **digitally silent** FLAC
  with generated cover art in five visual idioms, plus one deliberately
  untagged folder. Generated fresh, never `~/Music`.

The renders that survive are in [`prior-art/`](prior-art/) and are cited by
filename throughout. They are my own captures of GPL-3 software showing my own
generated artwork, so they are committed rather than linked.

| Peer | Version | Rendered | Why it earned the time |
|---|---|---|---|
| **fooyin** | 0.9.2 | 6 frames, all six shipped layout presets | `VISION.md` names it. The living heir to the Columns UI tradition, with a layout editor |
| **Strawberry** | 1.2.18 | 1 frame | Clementine's heir; the classic 3-pane, and it makes **Queue** a nav destination |
| **Lollypop** | 1.4.45 | 2 frames | The closest local-library precedent for an art-forward GNOME home |
| **Elisa** | 25.12.3 | — | **Would not start headless.** Recorded as a failure, not quietly dropped; its structure below is from source, not pixels |

### 0.2 Interfaces examined as images

Downloaded to a scratch directory and *looked at*, then measured — never
committed:

- **YouTube Music** web player, December 2023, 1920×1080, from
  [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Screenshot_of_YouTube_Music_web_player_(December_2023).png).
  Every proportion quoted for YouTube Music in §2 was measured off that file.
- **Amberol 0.10.3**, 1388×781, from
  [Commons](https://commons.wikimedia.org/wiki/File:Screenshot_of_Amberol_0.10.3.png).
  The colour-from-artwork evidence in §6.3 is measured off that file.
- Elisa, Clementine, JuK and Amarok reference shots, used only for the
  "what reads as dated" judgement in §6.2.

### 0.3 Interfaces studied from primary text, not pixels

Where a product is proprietary and its imagery is not obtainable legitimately,
the analysis carries the substance and the source is linked. Roon's queue
behaviour below is quoted from Roon's own knowledge base; Plexamp's design
rationale from its authors' own writing; Feishin's, Supersonic's, Museeks',
Amberol's and Elisa's layouts from the files that *define* those layouts
(`.qml`, `.blp`, `.tsx`, `.go`), which is stronger evidence than a screenshot
because it states the intent as well as the result.

### 0.4 What could not be verified, stated rather than hidden

- **Roon's Focus browsing.** Roon's KB article on the Queue fetched cleanly and
  is quoted verbatim in §5.1. The Focus articles 404'd on every slug tried, and
  the KB's own images are served from a dead S3 bucket (`roon-kb`,
  `NoSuchBucket`). Focus is therefore described in §3 only to the extent the
  Queue article and secondary sources support, and is **not** used to carry any
  recommendation.
- **Plexamp's current layout was not seen.** `plexamp.com` is a JS-rendered page
  that returns only its tagline to a fetcher, and the Plex support article on
  the Plexamp UI now 301s to the support index. What is covered is its authors'
  own design writing, its documented queue behaviour, its Play Queue API, and
  its community's complaints — enough to be useful, and enough to say something
  the audit does not (§5.1, §6.1, §7.3).
- **Audirvana is not covered.** The web-search budget (200 calls) was exhausted
  before it could be reached. That is a real gap against the brief and it is
  better said than papered over.
- Reddit, the Spotify community forums, `getmusicbee.com`, `forums.winamp.com`
  and several publishers blocked the fetcher outright. Where a claim rests on a
  search snippet or a thread *title* rather than a page read, it is marked
  *(snippet)* or *(title only)* at the point of use, and should be re-checked in
  a browser before it is quoted publicly. MusicBee's model below is sourced
  principally from its own plugin API header and its Miraheze wiki, which are
  primary and reachable.

---

## 1. The workflows, harvested from the products

Every interface encodes a bet about what people do most. The bet is legible in
what gets a permanent region, what gets a keystroke, and what is three clicks
into a menu. This section reads those bets off the products studied, rather than
asking our personas what they want.

### 1.1 The catalogue

Twenty-one workflows. The first fourteen were on the brief's list; **W15–W21 were
found by studying, which is the point of studying.**

| # | Workflow | Which product's design revealed it |
|---|---|---|
| **W1** | **Put on an album** — decide on a record, start it at track 1 | Longplay, Lollypop, Supersonic: the whole home screen exists for this |
| **W2** | **Resume what I was playing** | Apple Music persists the queue across quit; Feishin has `savePlayQueue`; Spotify restores |
| **W3** | **Find a specific thing I know I own** | Apple's ⌥⌘F filter field, distinct from global search; Tauon's type-anywhere |
| **W4** | **Browse to decide** — scroll a wall, no target in mind | Longplay, GOG Galaxy, Steam, Netflix, Lightroom grid |
| **W5** | **Play something without deciding** — shuffle / radio | Lollypop's "Random albums" nav item; Spotify's Smart Shuffle; Plexamp's mood radio |
| **W6** | **See what is coming next** | Every product in §5 has a dedicated affordance for this |
| **W7** | **Change what is coming next** — jump, remove, reorder | Roon's 3-bar reorder handle; Spotify's drag; Feishin's `Remove from queue` |
| **W8** | **Add to what is playing without losing it** — Play Next / Add to Queue | Apple's stack-of-contexts model; the single most-copied row verb in music software |
| **W9** | **Inspect a release** — tracklist, year, format, credits | fooyin's Selection Info panel; TIDAL's navigable credits; Calibre's Book details |
| **W10** | **Check or fix metadata** | Calibre's `cover:false`; Plex's fix-art-from-the-grid; MusicBee's tag tools |
| **W11** | **Adjust how it sounds** — output, exclusive mode, ReplayGain, DSP | TIDAL's format pill → spec on click; fooyin's Playback settings tree |
| **W12** | **Get back to what is playing after wandering** | Roon's "Jump to now playing"; Spotify's `Alt+Shift+J`; Apple's ⌘L; Tauon's right-click-play |
| **W13** | **Deal with a large library** — jump, filter, sort, facet | fooyin's Obsidian/Ember facet rails; Calibre's Tag browser; Apple's Column Browser |
| **W14** | **Manage where the music lives** — roots, rescan, watch folders | Every player; universally a settings surface |
| **W15** | **Compare two releases** — is this the remaster or the original? | Calibre's inspector-follows-selection; Lightroom's Compare (`C`) mode. *Marta's actual loop, and no music player supports it* |
| **W16** | **Verify the chain is honest** — is this bit-perfect right now? | TIDAL's pill by the scrubber; fooyin's Selection Info sample-rate row |
| **W17** | **Change density** — see more covers, or fewer and bigger | Lightroom's slider + `J` cycle; Plex's poster slider; Feishin's `itemSize`/`itemGap`/`itemsPerRow`; **Steam and Google Photos both took lasting damage for removing it** |
| **W18** | **Find the holes in the collection** — what is missing art, what is untagged | Calibre's `cover:false` search. *No music player does this* |
| **W19** | **Save the transient** — turn what is playing into something permanent | Roon's "save queue to playlist"; TIDAL's; YouTube Music's (with no undo) |
| **W20** | **Scope the library and stay scoped** — work inside a slice for a while | Calibre's Virtual Libraries; GOG's saved views; Steam's dynamic collections |
| **W21** | **Recover the layout after breaking it** | fooyin's `Ctrl+Z` in layout editing mode, and Import/Export layout as files. *A workflow that only exists because the product created the problem* |

W21 is the tell. A workflow whose only cause is a feature is an argument against
the feature — and it is the customisable-panel tradition's signature defect
(§4.3).

### 1.2 Frequency, for people who own their music

The layout must be optimised for frequency, not for importance. Something done
forty times a session must be nearly free; something done twice a year may cost
a click and a whole surface. Ranking from the personas
(`docs/research/05-personas.md`) and from what the products' own defaults imply:

| Band | Per session | Workflows | What the layout owes them |
|---|---|---|---|
| **A — constant** | 10–100× | W4 browse, W1 put on an album, W12 get back to what is playing | Zero clicks. Must be the resting state of the window, or one keystroke from it |
| **B — frequent** | 3–20× | W3 find a known thing, W9 inspect a release, W6 see what is next, W15 compare two releases | One click, no navigation, nothing else lost |
| **C — occasional** | 0–3× | W2 resume, W5 shuffle, W7 change what is next, W8 add without losing, W13 facet a large library, W17 change density | One click to reach, may cost a transient surface |
| **D — rare** | ~weekly | W16 verify the chain, W19 save the transient, W20 scope | May cost a layer down |
| **E — very rare** | ~monthly or less | W10 fix metadata, W11 adjust sound, W14 manage roots, W18 find holes | May cost a whole place. Should never cost the shelf a pixel at rest |

Three consequences follow immediately, and all three are arguments the current
specs make correctly:

1. **Band A is the shelf.** The window's resting state must be the collection,
   and the audit is right that this is the pillar.
2. **Band E must not be resident.** The current rail holds Settings — a band-E
   surface — in the same 340 px that band-B surfaces need. The audit's §1.3(c)
   makes this argument from keystroke frequency; the workflow ranking makes it
   independently and reaches the same place.
3. **W12 has no owner in baz today.** "Get back to what is playing" is a
   band-A workflow — Roon, Spotify, Apple and Tauon each spend a *dedicated
   affordance* on it — and baz has nothing. This is the largest gap the study
   found that neither existing spec names. See §8, R3.

---

## 2. The comparative matrix

### 2.1 Window division

Proportions marked ▣ were measured off pixels by me. Others are from the files
that define the layout, or from the vendor's own documentation.

| Product | Top chrome | Left | Centre | Right | Bottom | Home = |
|---|---|---|---|---|---|---|
| **baz today** (1280×860) | 56 px bar (6.5%) | shelf, 73% | — | rail 340 px (**26.6%**) | 102 px (11.9%) | **album shelf** |
| **YouTube Music** ▣ (1920×1080) | 80 px (7.4%) | nav 300 px (15.6%) | art 860 px sq | Up next / Lyrics / Related, 600 px (**31%**) | 80 px (7.4%) | feed |
| **Spotify** desktop | — | Your Library (resizable) | recommendations | Now Playing ⇄ Queue | persistent bar | recommendations |
| **Apple Music** (Tahoe 26) | — | scopes: Apple Music / Library / Playlists / Pins | content | slide-in: Playing Next *or* Lyrics | floating bar (moved from top in 2025) | Listen Now |
| **Roon** | — | browser | content | — | transport; queue opens from it | album browser |
| **fooyin "Simple"** ▣ | menu + **transport at top**, 32 px | Library Tree 304 px (19%), art below | — | playlist, 80% | — | **empty playlist** |
| **fooyin "Obsidian"** ▣ | search field | two facet rails, 420 px (**26%**) | playlist 804 px (50%) | art + metadata 344 px (21.5%) | transport | **empty playlist** |
| **fooyin "Ember"** ▣ | **four facet rails across the top, ~25% of height** | art + metadata | playlist | playlist organiser | — | **empty playlist** |
| **fooyin "Vision"** ▣ | — | **artwork, 51% of the window**; vertical tab rail | — | playlist, 49% | — | **empty playlist** |
| **Strawberry** ▣ | — | icon rail 78 px (4.9%) + collection 269 px (17%) | playlist, 78% | — | transport 45 px + status 25 px | collection tree |
| **Lollypop** ▣ | header bar 44 px, **transport top-left** | nav rail 213 px (13%) | album grid | — | — | Suggestions |
| **Elisa** | **HeaderBar ≈ 20% of window height, transport inside it** | ViewSelector | grid / list | playlist pane (animated width) | — | Now Playing |
| **Amberol** ▣ (1265×655) | titleless header | playlist 320 px (25%) | centred art 195 px + waveform + transport | — | — | *no library* |
| **Supersonic** | browser toolbar: back / forward / reload | browsing pane | — | sidebar: Play Queue / Lyrics | 3-part player bar | **album grid** |
| **Feishin** | configurable window bar | nav rail (collapsible, reorderable) | content | player sidebar: queue / lyrics / visualizer | player bar | carousel feed |
| **Nuclear** | search box | nav | content | **queue, always present** | player bar | Dashboard |
| **Museeks** | **transport + centred playing bar (max 600 px) + search, all at top** | icon-only nav strip | track table | — | — | track table |
| **Longplay** (iOS) | — | *(swipe left: sort orders, settings)* | **the album wall, full screen; a magnifying glass top-left is the only persistent control** | *(swipe right: Now Playing → queue)* | **none — no mini-player at all** | **the album wall** |
| **Longplay** (macOS) | sort options + playback controls | — | album wall | — | mini player from a toolbar indicator | album wall |
| **Doppler** (Mac) | toolbar: search + queue toggle | Library (Artists/Albums/Songs), Presets, Playlists | **album grid** | **the queue, toggleable** | docked transport | album grid |
| **Albums** (iOS) | sort/filter toolbar | — | **album art grid**, Quick Actions dock near the bottom | — | playback controls above a 5-tab bar | **album grid** |
| **Plexamp** | — | — | **recommendation shelves; the cover wall is one level down under Library** | — | player (*"the player is the primary user interface element, and it sits on top of everything else"*) | **shelves of recommendations** |
| **Calibre** | search bar | Tag browser (facets) | book list | **Book details inspector** | jobs | list / cover grid / shelf |
| **Lightroom Classic** | module picker | Catalog / Folders / Collections | grid or loupe | metadata / keywords | **filmstrip, persists in detail view** | grid |

### 2.2 The dimensions the brief asked for

| | Currently-playing item | Album / detail treatment | Search | Settings | Art |
|---|---|---|---|---|---|
| **baz today** | halo + dot on tile; dot in inspector row; bar names it | right inspector, 340–420 px | persistent field, top-left | **rail today, place in flight** | square, no radius, contact shadow |
| **YouTube Music** ▣ | raised row + ▶ thumbnail overlay in Up next; 48 px thumb in bar **duplicating the 860 px art on the same screen** | full-page player | centred field in top bar, 31% of width | avatar menu | rounded, **blurred enlarged copy of itself behind** |
| **Spotify** | row highlight | page | scoped field | avatar | rounded; gradient header from dominant colour |
| **Apple Music** | — | separate page per album (**6 per row, down from iTunes' 13**) | sidebar top, **defaults to catalogue not your library** | Music > Settings | **twisted+blurred copies in a Metal shader, not colour sampling** |
| **fooyin** ▣ | small ▶ glyph in a "Playing" column; **the transport carries no title or artist at all** | Selection Info panel: 20 fields incl. bit depth, sample rate, codec, tag types | **`Library > Search` — a menu item**, except in Obsidian/Ember | modal dialog, 25+ section tree | placeholder disc until something plays; art panel is bound to the *playing* track, not the selected one |
| **Strawberry** ▣ | — | Context tab | field above the collection tree | modal dialog | art appears in **three competing surfaces**, with code arbitrating between two |
| **Amberol** ▣ | speaker glyph on the row | *no album view* | icon in the playlist header | hamburger | **rounded card + shadow; whole window washed with a 3-gradient composite from the palette** |
| **Calibre** | — | right inspector, fields user-configurable | `/` or Ctrl+F, field syntax, regex | Preferences | grid size + background both user-set |
| **Lightroom** | — | Loupe mode, filmstrip retained | — | — | **thumbnail slider + `J` cycles three caption densities** |

### 2.3 Density, in one number each

Measured off my renders, as "fraction of the window given to the user's own
content at rest, with nothing playing":

| Product | Content share at rest | What the rest is |
|---|---|---|
| **fooyin "Simple"** ▣ | **19%** (library tree) | 80% empty playlist, saying "Playlist empty" |
| **fooyin "Obsidian"** ▣ | **26%** (facet rails) | 50% empty playlist, 21% empty inspector |
| **fooyin "Vision"** ▣ | **0%** — the library is a collapsed vertical tab | 51% placeholder disc, 49% empty playlist |
| **Strawberry** ▣ | **17%** (collection tree) | 78% empty playlist under a giant watermark logo |
| **Lollypop** ▣ | 87% (album grid area) | 13% nav rail |
| **baz today** | **73%** (shelf), 100% with the rail closed | 27% inspector |

**This is the single most important number in the document.** The tradition baz
succeeds gives the user's own collection between 0% and 26% of the window at
rest, and spends the majority on an empty container waiting to be filled. baz
gives it 73–100%. That is not a refinement of the tradition; it is a different
product, and it is the thing worth protecting above every other decision in
these three design documents.

---

## 3. Workflow by workflow, product by product

The comparative meat. Steps are counted from a resting window with nothing
playing, mouse only unless a shortcut is named.

### W1 — Put on an album

| Product | Steps | What is visible while doing it | Verdict |
|---|---|---|---|
| **baz today** | 1 (double-click a tile) or 2 (click, then `Play album`) | the shelf, reflowing once | **Best in the survey.** Increment 3 fixed the double-click that the reflow used to break |
| **Longplay** | 1 (tap a cover) | the cover wall | The proven pattern; the audit is right to cite it |
| **Lollypop** ▣ | 2 (nav to Artists/Albums, click) | grid | Fine |
| **Supersonic** | 2 (grid → album page → play) | album page | Fine |
| **fooyin** ▣ | **3+ and it does not play.** Double-clicking an album in the Album filter rail *appends it to a playlist* and starts nothing; a second double-click on a track row starts playback. My double-click appended the album **twice** — [`fooyin-06`](prior-art/fooyin-06-obsidian-album-loaded.png) shows tracks 1–10, 1–10 | a spreadsheet | **The defining failure of the tradition.** Browsing is decoupled from playing by a container the user must first fill |
| **Spotify** | 1 — **but only since Adele, in November 2021**, and Premium only. Before that the album page's primary button shuffled | recommendations | The default was wrong for a decade |

Spotify's own wording concedes it: the change to make Play the default on all
albums was *"long requested by both users and artists"*
([Spotify, via MusicTech](https://musictech.com/news/industry/adele-gets-spotify-to-remove-shuffle-button-30-album-because-art-tells-a-story/)).
Adele: *"We don't create albums with so much care and thought into our track
listing for no reason."* It took one of the most powerful artists alive to
change a default. baz gets this right by construction and should never
relitigate it.

### W4 — Browse to decide

Only **three** of sixteen open-source players surveyed open on a wall of album
covers, and two of those three are streaming-server clients (Supersonic, whose
`StartupPage()` returns `AlbumsRoute()`; Feishin, whose home is a carousel and
whose Albums page is the grid). Strip those out and **essentially nobody in
open-source local-library players opens on your own covers.** Tauon has the best
gallery in the field — a single click plays the album — and *still* lands you on
a tracklist; the gallery is a mode you enter with `Tab` or mouse button 4
([Tauon manual](https://tauonmusicbox.rocks/manual/gallery/)).

This is an unoccupied position, not a crowded one. It is baz's strongest
structural claim and nothing in this study contradicts it.

Outside music the position is *normal*: GOG Galaxy, Steam, Calibre's cover grid
and bookshelf views, Lightroom's grid, Plex, Netflix. Collection browsing is a
solved problem and music software is the laggard.

### W6 / W7 — See and change what is next

The whole of §5.

### W9 — Inspect a release

The cataloguer-audience products all converge on a **right-hand inspector that
follows selection**: Calibre's Book details panel, Lightroom's right panel,
Apple Photos' Info inspector, and — the one music example — fooyin's Selection
Info widget, which in my render carried twenty fields including *Bit Depth*,
*Sample Rate*, *Codec* and *Tag Types*
([`fooyin-06`](prior-art/fooyin-06-obsidian-album-loaded.png)).

The consumption-audience products all use **full-page navigation**: Steam, GOG,
Plex, Bandcamp, Apple Books, Apple Music (a separate page per album, and users
complain specifically that you *"can't view albums in place"*).

**The split is by audience, not by domain.** That is the finding that decides
baz's inspector-versus-page question, and it decides it in the audit's favour.
See §7.1.

### W12 — Get back to what is playing

The workflow baz has no answer to, and everyone else spends a dedicated
affordance on:

| Product | Affordance |
|---|---|
| **Roon** | a **"Jump to now playing"** button that appears in the queue when you scroll away, which *"will instantly reposition the view so that the currently playing track is once again at the head of the list"* ([Roon KB](https://help.roonlabs.com/portal/en/kb/articles/the-queue)) |
| **Spotify** | `Alt+Shift+J` ([keyboard shortcuts](https://support.spotify.com/us/article/keyboard-shortcuts/)) |
| **Apple Music** | ⌘L, "Go to Current Song" |
| **Tauon** | **right-click the play button** locates the playing track ([manual](https://tauonmusicbox.rocks/manual/interface/)) |
| **Elisa** | "Show Current" in the playlist toolbar |
| **baz** | **nothing** |

In baz the playing album is haloed and dotted on the shelf — which is the right
*mark* — but there is no way to scroll to it. In a 5,000-album shelf the mark is
invisible until you find it, and finding it is the workflow.

### W13 — Deal with a large library

fooyin's Obsidian and Ember presets are the tradition's answer and they are worth
looking at, because they are what "power" costs in this idiom:
[`fooyin-04`](prior-art/fooyin-04-obsidian-layout.png) spends 26% of the window
on two scrolling text rails (Album Artist, Album);
[`fooyin-05`](prior-art/fooyin-05-ember-layout.png) spends ~25% of the *height*
on four of them across the top (Genre, Album Artist, Artist, Album) and puts the
transport in the middle of the window. A 26-album library renders as two columns
of text. Zero covers appear anywhere except the now-playing panel.

Calibre does the same job better: a Tag browser with **counts that respect the
active restriction** (`set_restriction()` recomputes them), Saved Searches, User
Categories with `@`-prefixed hierarchies, and **Virtual Libraries** — named
persistent search restrictions whose user-facing framing is *"pretend that your
calibre library has only a few books instead of its full collection"*
([calibre manual](https://manual.calibre-ebook.com/gui.html)).

Steam's warning applies to whatever baz builds here: its multi-select is AND-only
with *"no option to do an 'Or' search"*, and filters applied to the sidebar
*"didn't also apply to the grid view"* — two filter models that disagree
([PC Gamer](https://www.pcgamer.com/steam-finally-lets-you-take-control-of-your-game-collection-but-needs-more-options/)).

### W17 — Change density

Nobody in music does this well; everybody outside music does. Lightroom has a
thumbnail slider, `-`/`+` keys, and `J` to cycle **Hide Extras → Compact Cells →
Expanded Cells**. Plex has a per-view poster-size slider. Feishin exposes
`itemSize`, `itemGap` and `itemsPerRow` in pixels. Calibre exposes cover size
*and* grid background.

And the two products that *removed* a density level both took durable damage:
Steam's grid-size slider (users demanded *"a slider to adjust icon size in GRID
MODE"*; Valve later restored a small library view) and Google Photos' year view
(*"collapsed each month into only 28 photos"* — its removal generated sustained
complaint). **Density is a first-class user control, not a designer's constant.**

`02-visual-language.md` §4.2 makes cell width a function of viewport width, which
is a genuine improvement — but it is still a constant from the user's point of
view. See §8, R7.

### W18 — Find the holes

Calibre makes incompleteness **queryable**: `cover:false` *"finds all books
without a cover"*, and covers are fixed by dragging an image onto the Book
details panel. Plex lets you fix artwork **from the grid**, by long-pressing the
poster, not only from the detail page ([Plex](https://support.plex.tv/articles/201272763-edit-details/)).

No music player in this survey does either. `05-personas.md` already anticipates
the affordance — *"gentle, dismissible improvement prompts ('142 albums missing
art')"* — and prior art says the better shape is a **facet**, not a prompt.
Marta wants to *go to* the holes, not be told about them.

---

## 4. The patterns that recur, and why

### 4.1 Load-bearing — conventions users already have

These recur because they encode something true, and their absence would read as
a bug rather than a choice.

1. **A persistent transport that never moves.** Universal. Every criticism found
   in this study is about *where* the transport went — Apple moved it from top to
   bottom in Tahoe 26 and Six Colors called the result *"cramped"* and
   *"partially obscured by content sliding behind it"* — never about whether it
   should be persistent. **baz's bottom bar is the best-argued surface in the
   product and the pixel-stability discipline is rarer than the specs claim.**

2. **Bottom-of-window transport.** Spotify, Apple (since 2025), TIDAL, YouTube
   Music, Supersonic, Feishin, Nuclear, Harmonoid, Strawberry, fooyin's Obsidian.
   Against: fooyin's Simple/Vision, Elisa, Lollypop, Museeks — all top. The
   top-mounted transport is now a minority and reads as "not a music player" to
   anyone arriving from a streaming service.

3. **Play Next / Add to Queue as universal row verbs.** In every product with a
   queue. Apple's implementation is the correct one and almost nobody copies it:
   the queue is a **stack of contexts**, so *"if you're listening to a playlist,
   you can choose an album to switch to after the song currently playing
   finishes. When the album finishes, Music resumes playing the playlist"*
   ([Apple](https://support.apple.com/guide/music/queue-your-songs-musb1e6d1c76/mac)).

4. **The primary action lives on the artwork, on hover.** Universal in the
   modern cluster. `02-visual-language.md` §3.1 forbids it — *"Nothing is ever
   drawn on top of a sleeve"* — which is a real and defensible break with
   convention, and baz pays for it with `Play album` in the inspector. Worth
   knowing it is a break. Notably, a search for prior *critique* of the
   play-button-on-artwork pattern returned nothing: baz's argument against it
   would be original rather than supported.

5. **A right-hand inspector for cataloguer audiences.** Calibre, Lightroom,
   Apple Photos, fooyin's Selection Info. §7.1.

6. **A queue affordance in or beside the transport, opening a right-hand
   surface.** §5.

7. **Scroll and selection survive a round trip to detail.** GOG's back button
   restoring scroll position is called out by name in reviews; Calibre shares one
   `BooksModel` across all four of its views with a `PreserveViewState` context
   manager so grid, list and shelf preserve selection and scroll; Discogs'
   failure to persist sort across a view switch is called out as a defect.

8. **A queue that survives quitting.** Apple documents it; Feishin has
   `savePlayQueue`. baz has nothing here — W2 is unimplemented.

9. **History and Up Next in one continuous scroll.** Apple only, and obviously
   correct: *"scroll up to the History section"*. Roon has the same instinct with
   the playing track pinned at the head.

### 4.2 Fashion — recurs but carries nothing

- **Colour-washed chrome sampled from the current cover.** Amberol washes the
  entire window; Spotify gradients the album header; Elisa, Feishin, Lollypop and
  Cider blur and dim the artwork behind chrome. It is *fashionable*, and it has a
  documented failure mode: Apple shipped adaptive colours on iOS 26.4 and
  *"screens can be very bright at night if you tap on an album that features
  mostly white or other light-colored backgrounds"*
  ([9to5Mac](https://9to5mac.com/2026/03/31/the-new-adaptive-apple-music-design-draws-complaints-from-dark-mode-users/)).
  In Tahoe, a translucent transport over artwork means *"a red album cover makes
  it look like repeat or shuffle is selected when it is not"*
  ([Apple Community](https://discussions.apple.com/thread/256142581), 67 "me
  too"s). **`02-visual-language.md` §3.3's discipline — hue only, lightness and
  chroma fixed, applied to one accent and never to a surface — is the correct
  reading of this evidence, and this study strengthens rather than weakens it.**

- **Rounded artwork.** Universal in the modern cluster; Amberol's rounded card
  with a soft shadow is a good example. baz keeps square corners because iced
  0.13 cannot clip an image *and* because sleeves are square. The rationalisation
  is honest and the result is defensible; it is a visible break with fashion, not
  with a load-bearing convention.

- **Blurred enlarged artwork as a backdrop.** YouTube Music, Elisa, Feishin,
  Cider. Cheap and never illegible. Apple's macOS full-screen player is the
  sophisticated version and it is worth knowing how it works, because it solves
  the problem without extraction at all: it *"notably does not work by sampling
  colors from the album art and blending them… the way they've constructed
  theirs is by layering copies of the artwork and 'twisting' each copy [then] a
  blur shader on top"* ([Aadish Verma](https://www.aadishv.dev/music), citing
  Apple designer Sam Henri Gold). Out of reach for iced 0.13 and correctly not
  attempted.

- **A carousel/feed home.** Feishin, Spotify, YouTube Music, Nuclear. Wrong for
  an ownership audience by construction — §4.3.

- **Layout customisation as an identity.** §4.3.

### 4.3 What the customisable-panel tradition got right, and wrong

baz's audience came from here, so this deserves the space.

**What it got right, and baz must not lose:**

- **Metadata is first-class and visible.** fooyin's Selection Info panel showed
  Artist / Title / Album / Date / Genre / Album Artist / Track Number / File
  Names / Folder Names / Total Size / Last Modified / Library / Tracks /
  Duration / Channels / Bit Depth / Avg. Bitrate / Sample Rate / Codec / Tag
  Types — twenty fields, in a resident panel, for free. baz's album inspector
  shows four lines. Karl and Marta came from a product that told them everything.
- **Facets over arbitrary fields.** Obsidian and Ember are ugly but they answer
  Marta's question in one click.
- **Configurations are shareable artefacts.** fooyin's `Layout → Import layout…`
  / `Export layout…` ([`fooyin-01`](prior-art/fooyin-01-first-run-layout-editing-mode.png)
  and the Layout menu) is the same culture as foobar2000's shared configs. This
  is a genuine community asset and the reason the tradition has lasted.
- **Nothing is hidden.** Every setting exists, in one tree.

**What it got wrong, first-hand:**

- **It asks the layout question before the music question.** fooyin's very first
  frame is a window titled **"fooyin — Layout Editing Mode"** behind a modal
  headed **"Quick Setup"** offering *Empty, Simple, Vision, Browser, Obsidian,
  Ember* — **with `Empty` preselected** — over a blank canvas reading *"Right-click
  to add a new widget"*
  ([`fooyin-01`](prior-art/fooyin-01-first-run-layout-editing-mode.png)). The
  first thing the product asks a new user is *which layout do you want*, and the
  default answer is *none*. This is `VISION.md`'s "configuration before
  usability" rendered as a screenshot.
- **Scanning your library puts nothing on screen.** After the scan found 26
  artists, 80% of the window still read **"Playlist empty"**
  ([`fooyin-02`](prior-art/fooyin-02-simple-scanned-playlist-empty.png)). The
  library and the thing that plays are different objects and the user must
  manually move records between them. This is the tradition's deepest structural
  choice and it is the one baz most needs to refuse.
- **Browsing creates clutter.** Double-clicking an album spawned a new playlist
  tab called *"Filter Results"* and appended the album twice
  ([`fooyin-06`](prior-art/fooyin-06-obsidian-album-loaded.png)).
- **The art-forward preset is not library-forward.** "Vision" gives 51% of the
  window to the *playing* artwork and collapses the library to a vertical tab; with
  nothing playing that is half a window of placeholder disc
  ([`fooyin-03`](prior-art/fooyin-03-vision-layout.png)).
- **The transport says nothing.** fooyin's default transport carries no title and
  no artist — only glyphs, timestamps and a groove. Knowing what is playing
  requires a separate optional widget.
- **Search is a menu item.** `Library → Search` / `Quick Search` in four of six
  presets; a persistent field exists only in Obsidian and Ember.
- **Where you are in the album is a preference.** `Playback → Cursor follows
  playback` and `Playback follows cursor` are unchecked checkboxes. The tradition
  makes "show me where I am" a setting.
- **W21 exists.** `Ctrl+Z` for layout edits.

The same disease in the neighbours: Strawberry ships **six sidebar display
modes** as a preference and renders album art in **three competing surfaces**
with code arbitrating between two of them; DeaDBeeF's `View → Design mode` has a
widget palette containing no now-playing widget, no queue widget and no search
widget; Quod Libet opens secondary browsers in **separate top-level windows**.

**And in foobar2000 itself, from its own documentation:**

- **Layout editing is a mode that steals your inputs.** Right-click stops
  meaning "act on this track" and starts meaning "select this element". Worse,
  the manipulation targets are invisible lines: *"To select a splitter, it's
  necessary to click on the border between its children"*, and to reach a parent
  you must right-click *"exactly on the splitter bar"*
  ([HA wiki](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:Layout_Editing_Mode)).
- **Operations are not invertible.** Removing an element *"will leave the
  splitted area intact"*, orphaning a container you must then separately find
  and replace — via the exact-pixel right-click above.
- **The primitive was under-powered for sixteen years.** DUI splitters were
  strictly binary until **v2.0, April 2023** (*"Improved Default UI splitter, now
  allows any number of panes"*). Dark mode arrived in the same release, twenty
  years after launch.
- **Two incompatible panel APIs meant every panel was written twice.** *"UI
  Elements are conceptually similar to the panels introduced by Columns UI, but
  are incompatible with them so they were given a new name."* The repository
  today holds 291 components, 22 tagged "Default UI element" and 28 "Columns UI
  panel".
- **The most-used feature of the most-used UI is undocumented.** Columns UI's
  archived layout page carries a `FIXME` saying the layout tree was never
  written up, and the *current* official docs have **no layout chapter at all**.
- **Configurations rot.** The wiki FAQ still points at gallery threads whose
  images were all replaced with *"The image link is no longer valid"* in a 2022
  moderator sweep, at a sibling thread that now 404s, and at a config database
  whose domain no longer resolves. Configs ship as **entire application
  profiles**, not settings.
- **The ecosystem breaks at ABI boundaries and users pay.** Facets — the best
  browser fb2k ever had — died at 32-bit in 2011 and was reimplemented in core
  as ReFacets **twelve years later**. Georgia-ReBORN, the flagship theme, filed
  *"64 Bit Compatibility"* on 2023-02-14 and closed it on **2025-09-02**: two and
  a half years in which the best-known foobar2000 theme could not run on the
  current foobar2000, because its author did not own the components it depended
  on.

**Two things from this tradition baz should take rather than refuse.**

**MusicBee's boundary, stated in its own documentation:**

> *"Skins can not affect the layout of panels, (global) fonts, or any
> functionality of MusicBee."*
> — [MusicBee wiki](https://musicbee.miraheze.org/wiki/Main/Theming)

Winamp let skins own everything, got 102,634 of them, half illegible, on
geometry frozen at 275×116 for the product's entire life. foobar2000 has no
skinning at all in its default UI, so *layout* became the medium of
self-expression — and layouts are exactly the thing that breaks on upgrade and
ships as a whole profile. MusicBee split them: **appearance is a semantic token
system (element × state × component, with 1×/150%/200% assets) that cannot break
structure; structure is a small fixed vocabulary that cannot be nested into
incomprehensibility, with the track list permanently in the centre.** That is
why its skins are safe to share.

`02-visual-language.md` has already drawn this line correctly without citing it —
`theme.rs` is a token sheet, and layout is code. Worth knowing the line has a
precedent, and that the two products that blurred it are the two that aged worst.

**And Winamp's windowshade**, which nobody has copied: 275×14 pixels retaining a
working transport, a 25×6 px time display that still toggles elapsed/remaining,
six micro transport buttons in 56 horizontal pixels, and a 17×7 px seek bar. An
*ambient* mode — the player reduced to a strip, still fully operable. Users still
miss it: *"the window-shade mode is nice for letting you see what's playing and
controlling with minimal distraction and taking up minimal space."* baz's
now-playing bar is already this shape; the idea worth borrowing is that the
player can become the *whole* window when the collection is not what you need.

**The synthesis: the tradition was right that power must be reachable and wrong
that layout is where power lives.** Power lives in *what the product knows about
your files* — the twenty-field readout, the facets, the tag tools. Layout
flexibility was the tax the tradition paid to deliver that, not the thing being
delivered. And the deepest structural finding is that **a panel model cannot
express a relationship**: a panel shows the selection or the playing track, and
"the item after the current one" is neither. That is why foobar2000 has no queue
view after twenty-four years and answers "what plays next" with *"not
possible"*. `VISION.md` pillar 6 already says power belongs one layer down; the
screenshots prove it, and the missing queue explains why it matters.

### 4.5 The Sonos warning, which bears directly on baz's places model

The audit's model is **named places**. In April 2024 Sonos shipped the opposite,
and the outcome is the best-documented interface failure in this study.

Sonos's own press release: *"Sonos Unveils Completely Reimagined Sonos App
Bringing Services, Content and System Controls to **One Customizable Home
Screen**"*, explicitly eliminating tab-jumping, with system controls moved to a
**swipe-up from the mini-player**. The prior structure — three named,
learnable destinations, **My Sonos / Browse / Rooms** — was removed entirely
([The Verge](https://www.theverge.com/2024/4/23/24137502/sonos-new-app-announced)).

What went with it: local music library configuration, browse, search and play;
queue editing; playlist editing; sleep timer; alarms; screen-reader support;
Play Next / Play Last / Shuffle All; and **alphabetical jump-to-letter**. The
Google Play rating fell to **1.3**.

Three things make this directly relevant to baz rather than merely cautionary:

1. **The owned-files cohort was hit hardest, and Sonos's own triage says so.**
   In the July 2024 apology the published roadmap's **first** item was
   *"Implementing Music Library configuration, browse, search, and play"* —
   ahead of volume responsiveness and ahead of restoring queue editing. A
   streaming user's content sits behind someone else's search box; a local-library
   user's content is reachable *only* through the app's own browse and search.
   Same code change, categorically different injury. From the community thread:
   *"I have a library of 40,000 songs all of which I have paid for but now
   unable to access."*

2. **The largest concrete regression for large libraries was losing
   jump-to-letter**: *"if you want something beginning with a 'T', you have to
   scroll through hundreds of screens"* with thousands of albums. **baz has no
   jump-to-letter and no index rail.** A wall of covers is beautiful at 200
   albums and unusable at 5,000 without one. This is the same finding as W12 and
   W13 arriving from a third direction.

3. **Removing named destinations removed the landmarks screen readers navigate
   by.** The IA decision and the accessibility failure were the same decision
   ([Mosen](https://mosen.org/sonos2024/)). baz already declares an accessibility
   gap (iced 0.13 publishes no accessibility tree); it should at least not
   *compound* it by making surfaces gesture-only.

The ending is the datum: **in July 2026 Sonos reintroduced bottom-tab
navigation — Home, System, Search — shipping it opt-in behind a setting**
([ecoustics](https://www.ecoustics.com/news/sonos-app-update-2026/)). Two years,
$20–30M in remediation, roughly 300 layoffs and a CEO resignation later, named
destinations came back.

**This is strong support for the audit's places model and against any drift
toward "everything is on the home screen, and everything else is behind a
gesture."** It is also a direct argument for R1's requirement that the queue
affordance be *visible and labelled*, not only a key and a click target.

### 4.4 Where the mainstream pattern actively fights baz's audience

1. **The library is a rail; recommendations are the room.** Spotify's own
   announcement anchors "Your Library" to the left rail and defines the centre —
   the largest region — as *"your central hub to browse, discover, and find
   recommended songs and podcasts"*
   ([Spotify](https://newsroom.spotify.com/2023-06-20/spotify-desktop-experience-redesign-your-library-now-playing-views-customize/)).
   TIDAL puts algorithmic "My Mix" *inside* My Collection. Apple pins an
   undisableable Apple Music section above Library while giving the *iTunes
   Store* — the part that sells you files — a hide toggle.

2. **Organising what you own is monetised.** Apple's Tahoe 26 headline library
   feature is Pins; *"After you subscribe to Apple Music, you can pin your
   favorite music… to the top of your library"*
   ([Apple](https://support.apple.com/guide/music/pin-music-mus953bc039a/1.6/mac/26)).
   Its justification — *"no more scrolling to find them"* — is an admission that
   library browsing broke at scale, answered with a subscription.

3. **Search defaults to the catalogue.** Apple's three-way scope selector
   (Apple Music / Your Library / iTunes Store) is structurally right and defaults
   wrong: typing an artist you own searches the shop.

3a. **The one consent pattern worth copying comes from a peer, not a giant.**
   Lollypop's first frame asks *"Automatically download albums and artists
   artwork?"* in a **dismissible banner over the content**, with `Yes` and `✕` —
   not a modal, not a wizard step, and not silent
   ([`lollypop-01`](prior-art/lollypop-01-first-run.png)). That is exactly the
   shape `05-personas.md` §4 specifies for baz's single enrichment prompt, and it
   is good to know a peer has shipped it. Note also what Lollypop's nav rail
   makes destinations: *Suggestions* is first and selected by default, above
   *Artists* and *Genres* — even a local-library GNOME player leads with
   suggestions rather than the collection.

4. **Your files are a tab.** YouTube Music segregates uploads under a separate
   *"Uploads"* tab in search results **and** in Songs/Albums/Artists, offers **no
   metadata editing at all**, sorts artist pages by `artist` rather than
   `album artist`, and explicitly excludes uploads from recommendations
   ([Android Police](https://www.androidpolice.com/2020/06/09/hands-on-youtube-music-upload/)).

5. **The queue-as-mutable-stream destroys the album-as-unit.** Spotify runs a
   playing *context* and a manual *queue* as two stacked structures, documents
   neither, and wipes the manual queue when you play anything new. Unfixed since
   at least 2016. The best statement of the failure is a Hacker News comment that
   reads as a specification for baz stated as a complaint:

   > *"Playing an album plays the first song in the album, and puts the rest in
   > the 'up next' part of the queue, but queueing an album queues all its songs
   > in the 'queue' part of the queue. 'up next' goes after 'queue', so this
   > means I will hear song A1, then B1, B2, […], then A2, A3"*
   > — [kroltan, HN](https://news.ycombinator.com/item?id=34259776)

6. **Visual calm is bought with control density, repeatedly, by everyone.**
   TIDAL's 2026 mobile redesign hid the progress bar (users needed *"to tap just
   to figure out what part of the song they're on"*), removed the *"Playing
   from"* indicator, and stripped skip from the miniplayer
   ([PiunikaWeb](https://piunikaweb.com/2026/05/06/tidal-ios-music-player-redesign-rolling-out/)).
   Spotify's move of the queue into the sidebar cost it **track duration, album
   name and saved-state**
   ([Windows Latest](https://www.windowslatest.com/2024/03/17/spotify-on-windows-11-gets-jam-and-moves-queue-to-the-right-side/)).
   YouTube Music shipped a thin miniplayer in March 2024 and re-added buttons by
   July. **Three vendors made the same mistake within two years, and the lost
   information was always position, provenance and skip.** This is the sharpest
   warning in the study for a design whose stated direction is restraint.

7. **Scroll latency.** *"It loads a single page of albums / songs / whatever at a
   time and takes a second to load the next one. This can turn the process of
   scrolling through your albums, which has been effortless since iTunes
   debuted, into a multi-minute process."*
   ([necubi, HN](https://news.ycombinator.com/item?id=24873575)). baz's
   virtualised shelf beats every streaming client on this for free, and it is a
   differentiator worth naming in the README.

---

## 5. Queue placement, specifically

The decision baz is making right now, and the one the parallel agent is
implementing.

### 5.1 The catalogue of solutions

| Placement | Products | What it accepts |
|---|---|---|
| **Affordance in/beside the transport → right-hand panel** | **Spotify** (icon in the bottom-right transport; *"locked to the right side, and you can't open it on the full screen"*), **Apple Music** (list glyph at the right of the player → slide-in right panel, sharing its slot with Lyrics), **TIDAL** (*"select [icon] at the bottom right of the Now Playing bar"*), **Doppler for Mac** (Queue button, top right; opens a right sidebar), **Supersonic** (right sidebar, `AppTabs` of Play Queue / Lyrics, split offset persisted), **Feishin** (`showQueueInSidebar`) | Ambient visibility; costs content width permanently, and the narrow column costs columns of data |
| **Affordance in the transport → transient popover** | **Museeks** — a list icon *inside* the centred playing bar, portalled to the `bottom` side, **only mounted when a track is playing**. **Harmonoid** — `DesktopNowPlayingPlaylist.show(context)`, an overlay | Costs the browse surface nothing; cannot be glanced at while browsing; no persistent home for W7 |
| **A child screen of Now Playing** | **Longplay** (wall → swipe → Now Playing → queue), **Doppler on iPhone** (mini bar → Now Playing → *"Up Next"*, bottom-left, **inline with the transport row**), **Albums** (platform Up Next; on Mac, **its own window**) | Contextually right, and cheap. Two steps from home |
| **Always-present right column** | **Nuclear** (nav │ content │ queue over a bottom bar) | Zero cost to reach; permanent width tax |
| **A destination in the nav rail** | **Strawberry** (Queue, sibling of Collection and Playlists), **Lollypop** ("Playing albums") | Cheap to build; W6 costs a navigation, and you lose your browsing position |
| **A full page you navigate to** | **Sonixd** (`/nowplaying`), **YouTube Music** desktop (the player *is* the page, Up next a tab within it) | Room for everything; checking what is next means leaving what you were browsing |
| **Docked below the browse list** | **Quod Libet** — a `Gtk.Expander` under the song list, with a count label and a **padlock** to disable the queue while still adding to it | Always visible; steals vertical space from the thing you are browsing |
| **A view opened from the bottom of the window** | **Roon** — *"clicking the queue icon at the bottom of the Roon window, or by clicking the currently playing song"*, opening a view with *"the currently playing track shown at the top of the list, and also in the panel above the queue list"*, plus a **"Jump to now playing"** button ([Roon KB](https://help.roonlabs.com/portal/en/kb/articles/the-queue)) | — |
| **Hidden behind an unlabelled gesture** | **Plexamp** — swipe the player up on mobile, **mouse-wheel down** on desktop; the App Store copy calls it *"play queue peeking"* | Zero chrome. **And it generates a permanent complaint stream** — §5.2(e) |
| **A drop target** | **Marvis Pro** — *"Use Drag & Drop to add one or more items to Up Next, Play, or Shuffle"*: three distinct drop semantics on one surface | Elegant on a pointer device; undiscoverable without a hint |
| **A widget you must place yourself** | **fooyin** — `queueviewer` with its own model, delegate, view and config widget; absent from every default preset, and **absent from the Playback menu entirely** (verified first-hand) | Maximum flexibility; the queue does not exist until the user builds it |
| **No view at all** | **foobar2000** — §5.2(b) | — |
| **None — the playlist *is* the queue** | **MusicBee** (one list with a cursor — §5.2c), **Amberol** (`queue.toggle` is bound to `playlist-visible`), **Elisa**, **Sayonara**, **DeaDBeeF** (a per-row marker), **Winamp** (2.x had no queue at all), **Strawberry** (`Queue : QAbstractProxyModel`, one per playlist) | No second concept to learn; but in most implementations, playing anything destroys your list |

### 5.2 What the evidence supports

**(a) The affordance belongs in or beside the transport, or on the now-playing
surface. Overwhelming.** Spotify, Apple, TIDAL, Doppler, Museeks, Supersonic,
Roon, Feishin, Longplay, Albums — ten independent products, three of them the
market leaders and three of them the album-first outliers, all put the door to
the queue either in the transport or one step into now-playing. The audit's
§2.3 argument — *"it should live next to the thing it describes"* — is the
majority position and is **confirmed**.

Corollary: **removing baz's top-bar `Queue · 13` toggle is right.** Nobody in
this survey puts the queue affordance in a library toolbar.

**(b) baz's own ancestor is the cautionary tale, and it is worse than the audit
knows.** foobar2000 has had **no built-in queue view in twenty-four years**. The
queue is a separate global structure whose only native representation is three
title-formatting fields (`%queue_index%`, `%queue_indexes%`, `%queue_total%`) —
you must author a custom playlist column to see it at all. Worse, the defaults
are hostile: the `foo_keep_queue` component exists solely to *"prevent the
playback queue from being removed when changing song manually, and save the
queue when restarting"*, which means **by default, picking a track by hand
flushes your queue, and the queue does not survive a restart**. And the wiki FAQ
answers "what plays next?" with: *"This is not possible in foobar2000 since
v0.9.5.3."*

A user asking for it in 2018 put it exactly right:

> *"It seem strange not to know whats going to play next, if you've forgotten
> what you or someone else added to the queue."*
> — [Hydrogenaudio](https://hydrogenaud.io/index.php?topic=115857.0); the sole
> reply is a link to a third-party component

**baz's audience arrives from a product where the queue was invisible, hostile
and only fixable by plugin.** Almost anything baz ships is an improvement — but
it also means baz cannot assume this audience has habits here. It has scar
tissue.

**(c) MusicBee solved it, and the model is the best in the survey.** There is
**one list with a cursor**. Its own plugin API states it plainly: `NowPlayingList_PlayNow`,
`NowPlayingList_QueueNext`, `NowPlayingList_QueueLast`,
`NowPlayingList_IsAnyPriorTracks`, `NowPlayingList_IsAnyFollowingTracks` — three
insert positions into *the same* list, with everything before the cursor being
history and everything after being the queue. Saved playlists are a wholly
separate API family. The list is a dockable panel, on by default in the right
sidebar, where *"queued tracks are marked with their position in the queue and
skipped tracks are marked with a minus sign"*, and clicking the header toggles
total time to **time remaining**.

This is Apple's stack-of-contexts idea in a simpler form, and it is why "Play
Now / Queue Next / Queue Last" reads as coherent in MusicBee and incoherent in
foobar2000 and Spotify: in MusicBee those are three positions in one visible
list; elsewhere they span two structures with different lifetimes that nobody
documents.

**(d) The strongest single data point is a reversal, and it is from the closest
prior art there is.** **Longplay shipped 1.0 (Aug 2020) with no queue at all**
and **stopped dead at the end of every album** — the maximalist album-first
position, exactly the one `VISION.md` flirts with in "queues are transient". Its
developer reversed that in 2.0 (Aug 2023), not because users demanded a track
queue, but because **the album boundary was a dead stop** that broke the flow.
The fix kept the album as the unit and offered two continuations: **Infinite
Album Shuffle** (automatic) or the **Album Queue** (manual, `Play Next` /
`Play Later` from a long-press, drag-to-reorder).

Sonixd → Feishin is the second such reversal: an author who shipped the queue as
a **page** at `/nowplaying` rewrote the whole application, and in the rewrite the
queue became **persistent, resizable right-hand chrome**. **Nobody in this
survey moved from a persistent surface toward a hidden one.**

**(e) A queue you cannot verify is worse than one you cannot reach.** Plexamp
hides its queue behind an unlabelled swipe-up (mobile) or mouse-wheel-down
(desktop), and the preview height is bounded by the window. The result is a
continuous, years-long complaint stream — *"'Up Next' list preview way too
short"*, *"Add to Queue, despite the icon, adds the track Next rather than…"*,
*"Up Next Confusion"*, *"Why is Plexamp's queue so unpredictable?"* *(titles
only; Reddit blocked the fetcher)*. Plex's own documentation states the insert
semantics clearly; the **UI makes the distinction illegible at the moment of the
gesture and unverifiable afterwards**. That is the failure mode, and it is
precisely the risk a transient overlay runs.

**(f) Narrow queue surfaces lose information, and users notice.** Spotify's move
into the sidebar cost it track duration, album name and saved state. The user
complaint that best captures why the queue matters at all:
*"I don't want to see what I could play, once I have chosen a playlist, I want to
see what will play."*

**(g) The album-vs-track queue dilemma has already been dissolved.** Longplay's
queue holds **albums**. Doppler's holds tracks but accepts **whole albums as
queue targets**. And **Albums (iOS) v7.1.1 ships a two-level queue**: albums
appear as rows, *"you can expand an album in the queue to see its tracklist, and
swipe to remove individual tracks"*. That is the shape an album-first player
wants, and it is already proven.

**(h) Model the queue as *what you chose*, not as the tracks that resulted.**
Plex's Play Queue is a server object carrying `playQueueID`, `playQueueVersion`
("increments every time a change is made") and — the useful one —
**`playQueueSourceURI`, the original request that created the queue**. So the
system knows you are playing *In Rainbows*, not "ten tracks that happen to be
from In Rainbows". Plexamp's **Recent Plays vs History** split (what you
*started* versus what actually *played*) falls straight out of that model, and
it is exactly right for an album-first player.

### 5.3 Verdict on baz's popover

**The popover is supported, and it is the right call *for baz specifically*,
for a reason the audit states but under-weights: baz's content is covers.**

Every product that spends permanent width on a queue spends it on a *list* —
Spotify's centre column is recommendations, Supersonic's browsing pane is a grid
it is happy to narrow, Nuclear's content is a web-style page. baz's centre column
is the pillar. A 340 px permanent queue would cost two shelf columns forever, to
serve a band-C workflow. The audit's arithmetic (§1.3(d)) is right and the
workflow ranking in §1.2 confirms it independently: **W6 is band C; the shelf is
band A; a band-C surface may not tax a band-A surface at rest.**

Longplay and Doppler-on-iPhone independently reach the same place from the
album-first side: the queue as a *child of now-playing*, never a peer of the
library, and **never a permanent tax on the wall of covers**. That is three
album-first products and baz agreeing.

But five refinements follow from the evidence. Two of them contradict the spec
as written, and one of them is the single most important sentence in this
document:

1. **Transient must not mean invisible or unverifiable.** Plexamp is the
   counter-example and it is the closest product to baz in ambition. The popover
   must be reachable by a *labelled, visible* affordance — not only by `Q` and a
   click on the now-playing block — and it must be tall enough to show what you
   just did. `POPOVER_W` 360 with `0.6 × window height` is adequate at 860 px
   (about twelve rows); it must not be reduced.

2. **The popover must not be the only home for W7.** Reordering, removing and
   context-stacking are what a queue surface is *for*, and a 360 px transient
   overlay is a poor place to do them. **Specify the growth path now**: when the
   queue stops being an album — after shuffle and radio land, which `VISION.md`
   v0.3 commits to — the popover gains a *"Open queue"* affordance to a full
   surface. Do not build it yet; do name it, the way the audit names the
   album-becomes-a-place path.

3. **Adopt MusicBee's one-list-with-a-cursor model explicitly, and say so in the
   spec.** History behind the cursor, queue ahead, one surface, three insert
   positions. It is simpler than Spotify's two-structure mess, simpler than
   Apple's stack (while getting most of its benefit), and it is the model baz's
   own audience already knows from MusicBee. The summary line should then say
   what *remains*, as MusicBee's header and Elisa's *"%1/%2 tracks remaining"*
   both do — not `3 of 12 · 51:20`, which describes the whole queue.

4. **Make the rows two-level when the queue stops being one album.** Albums'
   collapsible album rows are the proven answer, and adopting the *shape* now
   costs nothing while there is only ever one album in the queue.

5. **The popover must open with a stopped engine.** Museeks mounts its queue
   popover **only when a track is playing**; baz's `Q` must not, or W2 (resume)
   has no surface and a key sometimes does nothing — the kind of conditional the
   audit's whole model exists to eliminate.

**And one thing the audit gets wrong that prior art is unanimous about.** §2.5
argues the queue popover becomes optional for Devon once the inspector marks the
playing track — true, shipped, and excellent. But the audit then treats *"what
is next"* as the queue's only job. It has a second job no other surface can do,
and every product in §5.1 spends an affordance on it: **W12, get back to what is
playing.** Roon's "Jump to now playing" lives *in the queue*. Spotify's
`Alt+Shift+J`, Apple's ⌘L, Tauon's right-click-play all exist because a listener
who has wandered needs a way home. In baz the wandering is through a wall of ten
thousand covers, so the need is *stronger*, not weaker. See §8, R3.

### 5.4 The album-boundary question, which neither spec answers

Longplay's reversal exposes a decision baz has not made and will be forced to:
**what happens when an album ends?**

- **Longplay 1.0**: nothing. Silence. The developer came to see this as breaking
  the flow and fixed it in the next major version.
- **Longplay 2.0+**: an explicit choice between *Infinite Album Shuffle*
  (automatic continuation) and the *Album Queue* (manual).
- **Plexamp**: `Autoplay`, a continuation policy with user-selectable modes,
  combined with `playQueueSourceURI` so the policy knows what you originally
  asked for.
- **baz today**: `QueueEnded`, and the bar reads *Nothing playing*.

**Every album-first product has had to solve this, and none solved it by doing
nothing.** baz's current behaviour is Longplay 1.0's, and Longplay's developer
already ran that experiment for us. The answer does not need to ship in v0.1 —
but it should be *named* as a policy with a legible setting rather than
discovered later as a bug report, because the two candidate answers (stop; or
continue by some rule) have different homes in the IA, and one of them needs
`VISION.md`'s steered shuffle to exist first.

Worth pairing with Longplay's **Album Purity Mode** — *"Next/previous buttons
move between albums, not tracks. No track shuffle"* — which ships the
maximalist position as an **opt-in setting rather than a default**. That is the
right shape for a product whose audience contains both Devon and Priya.

---

## 6. What "modern and sleek" means in 2026

Not adjectives. What current products actually do, and what they have stopped
doing.

### 6.1 What the modern cluster does

Read off Amberol, Feishin, Supersonic, Elisa, Tauon's 2026 releases, Lollypop and
the four streaming clients — the traits co-occur almost perfectly:

1. **No menu bar.** A header bar or nothing. Every dated product in §6.2 has one;
   no modern one does.
2. **A persistent bottom now-playing bar**, three-zone: what is playing / transport
   over a groove / auxiliary controls.
3. **An art-forward browse surface** — grid or cards, not uniform text rows.
4. **A layout that reflows at narrow widths**, with a real breakpoint. Amberol's
   `Adw.OverlaySplitView` docks the sidebar when wide and overlays it when narrow;
   Feishin's full-screen player restacks from two columns to two rows in portrait;
   Elisa runs the same codebase down to Plasma Mobile.
5. **Whitespace and a wide type scale** — a real difference between a title and a
   caption, not one step of grey.
6. **A single accent, or none.** Nuclear ships six accent themes; Feishin makes
   accent a chosen colour with a shade ramp.
7. **Subtle, purposeful motion.** Elisa cross-fades cover art between tracks with
   an incubated-then-swapped image; Supersonic cross-fades two `canvas.Image`
   layers; calibre animates a cover swap over 1000 ms with exponential easing
   *and skips it when the book is unchanged*.
8. **Density as a user control** (§3, W17).

Plexamp's authors, on the discipline that produced what everyone calls the bar:
*"we even forced ourselves to limit the design to a single simple window"*, an
app that *"sits unobtrusively on a desktop, beguiling and delighting"*, with
*"soft transitions"* on pause and seek, and — the detail worth stealing —
*"those three little animated bars which show the currently playing track in the
play queue? That's actually a working spectrum analyzer"*
([Plex Labs](https://medium.com/plexlabs/introducing-plexamp-9493a658847a)).
That is a product where the *marking of the playing row* got real design
attention. baz's lamp dot is the same instinct, correctly scoped to a toolkit
that cannot animate.

### 6.2 What reads as dated, concretely

Four traits, and they travel together — I checked them against five products and
the correlation is near-perfect:

| Trait | Strawberry | Quod Libet | DeaDBeeF | Sayonara | fooyin |
|---|:-:|:-:|:-:|:-:|:-:|
| A menu bar at the top | ✓ | ✓ | ✓ | ✓ | ✓ |
| A status bar rather than a now-playing bar | | ✓ | ✓ | | |
| Primary browse surface is uniform text rows with no art | ✓ | ✓ | ✓ | ✓ | ✓ |
| Visual identity entirely inherited from the OS widget style | ✓ | ✓ | ✓ | ✓ | ✓ |

Plus the specific tells: Strawberry's tiled decorative `sidebar-background.png`
behind a 32 px raster icon strip, and its **six sidebar display modes offered as
a preference** — a 2010-era answer to "we couldn't decide"; its giant watermark
logo filling the empty playlist ([`strawberry-01`](prior-art/strawberry-01-collection-empty.png));
Quod Libet's newest official screenshots dated 2017; DeaDBeeF's gallery dated
2021 and a widget palette with no search widget.

**And the middle case, which is the one baz must design against.** Supersonic is
structurally fully modern — browser-chrome toolbar with back/forward/reload, an
album grid as the default page, a right sidebar with Play Queue and Lyrics tabs,
a three-part player bar, a dominant-colour Now Playing page. It still does not
read as *designed*, because Fyne supplies the entire visual language: uniform
rounded rectangles, uniform padding, a narrow type scale. Its theme format is a
fixed TOML slot list — a theme can change colours but **cannot change form**.

*Correct structure, generic surface* is a more likely failure mode for baz than
Strawberry's datedness, precisely because iced ships no widget style at all.
`02-visual-language.md` §2.2's decision to bundle IBM Plex is the single most
important defence against it, and this study raises rather than lowers my
confidence in it.

### 6.3 Honest audit — is anything in baz dated?

Going trait by trait against §6.2 and §6.1:

| Check | baz | Verdict |
|---|---|---|
| Menu bar | none | Clean |
| Status bar vs now-playing bar | a real 102 px three-zone bar | Clean, and better than most: nothing in it moves |
| Text rows vs art | a wall of covers at 73–100% of the window | **Ahead of every peer** (§2.3) |
| Identity inherited from the OS | **was true; fixed in `a729c09`** by bundling IBM Plex | Was the single most dated thing about baz; resolved |
| Reflows at narrow widths | specified in `01` §4.3, `< 940 px` regime not yet built | In flight |
| Whitespace, type scale | six-step scale, base-4 ladder, one serif accent | Clean |
| Single accent | one, reserved to playback truth, enforced in unit tests | **Ahead of the field.** No peer enforces its accent discipline in tests |
| Motion | 0 ms everywhere, argued from the toolkit and the performance budget | Defensible, and honestly the weakest column against the modern cluster |
| Density control | cell width is a function of viewport; **the user has no control** | **A gap.** §8, R6 |

**Three things I would flag as dated or at-risk, and only three:**

1. **Six sidebar display modes** is Strawberry's disease and baz does not have
   it. But `01` §4.7 preserves `Ctrl+B` to hide the inspector *and* `Esc` *and*
   ✕ *and* clicking the tile again — four ways to dismiss one panel. That is the
   same instinct in miniature. Prior art suggests two (a click-target and `Esc`)
   and no more.
2. **0 ms motion.** Defensible, well-argued, and the one place baz will read as
   less finished than Amberol or Elisa to anyone comparing side by side. It is
   the right call for iced 0.13; it should be revisited the moment the toolkit
   allows, and `02` §2.6 already specifies exactly what would animate. Nothing to
   change; worth knowing.
3. **No user-controlled density.** The one modern-cluster trait baz is missing
   outright, and the one whose removal elsewhere caused durable complaint.

Nothing else in either spec falls into the dated column. The palette, the
single-accent discipline, the reserved-slot bar and the square sleeves are all
either current or deliberate.

---

## 7. Verdict on baz's proposed IA

Against `01-ux-audit-and-ia.md` §2: **one place at a time, one inspector
attached to it, one popover attached to the transport, and the bar always.**

### 7.1 What prior art supports

**The album as a right-hand inspector, not a page — strongly supported, and the
support is more specific than the audit knew.** Tallying grid→detail across the
collection browsers:

| Pattern | Products |
|---|---|
| **Right-hand inspector following selection** | Calibre, Lightroom Classic, Apple Photos, fooyin's Selection Info |
| **Full-page navigation** | GOG Galaxy, Steam, Plex, Discogs, Bandcamp, Apple Books, Apple Music |
| **Modal / lightbox overlay** | Netflix, Google Photos, Criterion |

**The split is by audience, not by domain.** Every product built for a
meticulous cataloguer runs an inspector; every product built for consumption
runs a page or a modal. baz's audience is the first group, and the audit reached
the right answer from Marta's click-the-next-sleeve loop without knowing that
Calibre, Lightroom and Apple Photos had all reached it before.

Two supporting details worth having:

- Cloudscape states the trade-off explicitly: split view is for *"browsing and
  inspecting selected resources within a collection"*, a details page for
  *"comprehensive analysis of a single resource"*, and — the sentence that
  vindicates the audit's deferred promotion path — **"split view is not a
  replacement of details page"**
  ([Cloudscape](https://cloudscape.design/patterns/resource-management/view/split-view/)).
- Steam's most-quoted criticism is a warning against the middle: *"the new UI
  adds grid and detail together, not really satisfying either users"*. baz avoids
  it by committing to the inspector at wide widths and to replacement below
  940 px, which is a choice rather than a compromise.

**The queue anchored to the transport — supported.** §5.2(a).

**Settings as a place, not a panel — supported and unanimous.** Every product
surveyed puts settings in a modal dialog (fooyin, Strawberry, Supersonic) or a
full page (Feishin, Sonixd, Spotify, TIDAL). Nobody puts settings in the same
slot as content. fooyin's tree has 25+ sections; Feishin's has five tabs and
eighteen sections. The audit's argument that the coming output chain *"does not
fit in one [column] at all"* is confirmed by every peer that has built it.

**Removing the top-bar Queue toggle — supported.** §5.2(a).

**One dismissal rule per layer — supported by the absence of counter-examples.**
The dated cluster's characteristic defect is offering several ways to do one
thing as a preference.

### 7.2 What prior art contradicts, or complicates

1. **W12 has no owner.** §5.3. The most-supported affordance in the study is
   missing from both specs.

2. **The album inspector shows four lines where the tradition showed twenty.**
   baz's audience arrives from a product that put Bit Depth, Sample Rate, Codec
   and Tag Types in a resident panel for free. `01` §4.5 specifies title /
   artist / meta / encoding, and `02` §4.3 the same. That is a real regression
   against fooyin for Karl and Marta, and neither spec acknowledges it. TIDAL
   shows what the modern version looks like: credits as a *navigable dimension*,
   where tapping a role *"will filter the track list to display the songs on
   which that artist has contributed towards in that same capacity"*
   ([PR Newswire](https://www.prnewswire.com/news-releases/tidal-launches-enhanced-credits-feature-to-spotlight-all-the-individuals-creating-music-300879572.html)).

3. **No user-controlled density.** §6.3.

4. **The playing-track mark is right; the way home is missing.** §5.3.

5. **A fourth structural option exists that neither spec considers, and it is
   the one both cataloguer-grade products chose.** Lightroom's Loupe is a *mode*,
   not a page: the detail takes the full canvas while the **Filmstrip persists
   along the bottom**, so the collection is never off-screen, and `G`/`E` round-trips
   in one keystroke. Calibre achieves the same continuity differently: all four
   of its views share one `BooksModel` with a `PreserveViewState` context manager
   so selection and scroll survive a view switch.

   This matters for the audit's **`< 940 px` regime**, where the inspector
   replaces the shelf entirely and the shelf — the identity — vanishes. Prior art
   says: keep a strip. It also matters for the eventual promotion of the album to
   a Place, which is where the audit says it is heading.

### 7.3 What baz would be inventing, flagged as risk

Inventions, in decreasing order of risk:

| Invention | Who else does it | Risk |
|---|---|---|
| **No permanent home for the queue at all** — a transient popover as the *only* queue surface | Museeks (popover, but mounted only while playing); Harmonoid (overlay); Longplay and Doppler-on-iPhone reach the same place as a child of now-playing. **But nobody ships a transient-only queue for a product that will later have shuffle and radio** | **Medium.** Lower than I first judged it — three album-first products agree the queue is not a peer of the library. Still wrong once `VISION.md` v0.3 lands, and Plexamp shows the failure mode. §8, R2 |
| **Stopping dead at the end of an album** | Longplay 1.0 did exactly this and **reversed it in the next major version** | **High, and it is a live defect rather than a design choice.** §5.4, §8, R4 |
| **Nothing ever drawn on top of a sleeve** — no hover play, no badge, no overlay | Nobody in this survey | **Medium.** Deliberate and well-argued, but it is a genuine break with a load-bearing convention, and no prior *critique* of the pattern exists to lean on. baz's argument here would be original |
| **The bar carries no artwork** | Almost nobody — YouTube Music puts a 48 px thumbnail in the bar while showing the same cover at 860 px on the same screen | **Low.** baz's reasoning is better than the convention, and the convention here is thoughtless duplication |
| **Hue-only extraction: lightness and chroma fixed, applied to one 6 px dot and one 4 px rail** | Nobody. Amberol takes a five-colour palette to the whole window; Supersonic and Harmonoid recolour one surface; Google desaturates and biases dark; Apple avoids sampling entirely | **Low, and this is the good kind of invention.** It is the only approach in the survey that cannot produce the documented failure modes (Apple's bright-at-night, Tahoe's disguised toggle states), because no foreground token is ever derived from artwork |
| **0 ms motion everywhere** | Nobody | **Low.** Toolkit-forced, honestly argued, reversible |
| **Square sleeves** | Nobody in the modern cluster | **Low.** Truthful and defensible |

The pattern worth noticing: **baz's inventions in the visual language are safe,
and its invention in the information architecture is the risky one.** A
transient-only queue is the one place where "a convention nobody uses is usually
unused for a reason" bites.

---

## 8. Recommendations

Prioritised by (workflow frequency × evidence strength ÷ cost). Each marked
against what is being built.

### R1 — Keep the queue popover, but give it a visible, labelled affordance
**CONFIRMS increment 6, with one addition.** Ten products put the queue door in
or beside the transport, or one step into now-playing (§5.2a); baz's centre
column is covers, so a permanent panel would tax a band-A surface for a band-C
workflow (§1.2). Longplay, Doppler-on-iPhone and Museeks all agree.

The addition is not optional. **Plexamp hides the same surface behind an
unlabelled gesture and has generated years of "where is my queue / what did I
just do" complaints** (§5.2e), and Sonos's gesture-first redesign was reversed
after two years (§4.5). `Q` and a click on the now-playing block are not enough:
the bar needs a **visible affordance that says what it opens**, and the popover
must be tall enough to verify what you just did — keep `0.6 × window height`.
Remove the top-bar toggle as specified. And the popover must open with a stopped
engine, unlike Museeks.

### R2 — Name the queue's growth path in the spec, now
**REFINES increment 6.** Two independent reversals run from hidden toward
persistent: Sonixd → Feishin (page → resizable right chrome) and Longplay 1.0 →
2.0 (no queue → an album queue). A 360 px overlay is the right home for an album
queue and the wrong home for a shuffle queue you are steering. Add a paragraph
to `01` §2.3 in the shape of the album-becomes-a-Place paragraph: *when the queue
stops being an album, the popover gains a door to a full surface.* No code today.

### R3 — Give W12 an owner: "back to what is playing"
**ADDS — neither spec has this.** Band A, the most-supported affordance in the
study, and absent from baz. Roon puts *"Jump to now playing"* in the queue;
Spotify binds `Alt+Shift+J`; Apple ⌘L; Tauon right-click-play. Cheapest correct
version: **clicking the bar's now-playing block scrolls the shelf to the playing
album and selects it**, and the same on a key. The audit already gives that block
a click target for the popover, so this needs a second gesture or a second
target — resolve it deliberately rather than by accident.

### R4 — Decide what happens when an album ends, and make it a legible setting
**ADDS — neither spec answers this, and it is a live defect.** baz stops dead;
that is exactly Longplay 1.0, which its developer reversed within one major
version because the album boundary broke the flow (§5.4). Every album-first
product has had to solve it. The policy does not need to ship in v0.1, but it
should be *named* now, because the two candidate answers have different homes in
the IA and one of them needs steered shuffle to exist. Pair it with Longplay's
**Album Purity Mode** shape: ship the maximalist position (next/previous move
between *albums*) as an **opt-in setting, not a default** — baz's audience
contains both Devon and Priya.

### R5 — Adopt MusicBee's one-list-with-a-cursor model, explicitly
**REFINES increment 6/7.** History behind the cursor, queue ahead, one surface,
three insert positions (`Play Now` / `Queue Next` / `Queue Last`). Simpler than
Spotify's undocumented two-structure mess, most of the benefit of Apple's
stack-of-contexts, and **it is the model baz's own audience already knows** — a
large share of them came to MusicBee precisely because foobar2000 had no queue
view at all (§5.2b, §5.2c). Two consequences for the spec as written:
the summary line should say what **remains** (MusicBee's header toggle, Elisa's
*"%1/%2 tracks remaining"*), not `3 of 12 · 51:20`; and rows above the playing
one are history, which the spec's `PAPER_FAINT` treatment already gets right.
When the queue stops being one album, make rows **two-level** — albums
collapsible to tracks, as Albums (iOS) ships today (§5.2g).

### R6 — Grow the album inspector's metadata, and give it somewhere to grow
**REFINES `01` §4.5 and `02` §4.3.** baz's audience came from a product that
showed twenty fields for free (§4.3). Four lines is a regression for Karl and
Marta. Near term: add the fields the scanner already has. Longer term: TIDAL's
navigable-credits model is the modern shape — a role is a link that filters the
library.

### R7 — Ship a density control
**CONTRADICTS `02` §4.2 as written.** The spec makes cell width a function of
viewport width, which fixes dead gutters but leaves the user with no control.
Every collection browser outside music has one; **Steam and Google Photos both
took durable reputational damage for removing one** (§3, W17). Cheapest version
that discharges the evidence: three named sizes (Comfortable / Regular /
Compact) mapping to `ART_MIN` of 160 / 200 / 256, in Settings → Appearance,
where `01` §4.5 already reserves the section. This does not conflict with the
viewport-derived width; it parameterises it.

### R8 — Give the shelf an index: jump-to-letter and type-to-jump
**ADDS.** The single most concrete regression Sonos users named was losing
alphabetical jump — *"if you want something beginning with a 'T', you have to
scroll through hundreds of screens"* with thousands of albums (§4.5). Longplay's
current release notes, five years in, are still about *"performance
optimization for large music libraries"*. **A wall of covers is beautiful at 200
albums and unusable at 5,000 without an index.** baz's fixture is 29 albums and
Marta's library is 40,000; this gap is invisible in every screenshot taken so
far. It also partly serves W12 and W13. Note the tension the audit already
resolved in §4.8: type-ahead cannot coexist with bare-letter transport bindings,
so the index must be a rail or a `/`-scoped behaviour, not type-anywhere.

### R9 — Keep a strip of the shelf in the `< 940 px` regime
**REFINES `01` §4.3.** Both cataloguer-grade products keep the collection
on-screen in detail view — Lightroom's Filmstrip, Calibre's shared model. The
current spec has the shelf vanish entirely below 940 px, which is where the
eventual full-window Album place is prototyped, so the decision propagates.
A single row of sleeves along the bottom preserves W4 and W15 at every width.

### R10 — Make missing art a facet, not a notification
**REFINES `05-personas.md` §3 and `01` §1.2.** Calibre's `cover:false` is the
proven shape; Plex lets you fix art from the grid. Marta wants to *go to* the
holes. When the shelf gains facets, "no artwork" and "untagged" should be among
the first.

### R11 — Do not buy calm with control density
**CONFIRMS, as a standing constraint.** Three vendors made this mistake within
two years and the lost information was always position, provenance and skip
(§4.4.6). baz's reserved-slot discipline is the structural defence and it is
already tested. Treat the bar's slots as a ratchet: a slot may be added, never
removed for tidiness.

### R12 — Reduce the inspector's dismissal gestures from four to two
**REFINES `01` §4.7/4.8.** ✕, `Esc`, `Ctrl+B`, and clicking the tile again is
Strawberry's six-sidebar-modes instinct in miniature (§6.3). Keep ✕ and `Esc`;
keep `Ctrl+B` only if it is genuinely a different intent (hide but remember).
Drop click-the-tile-again, which collides with double-click-to-play.

### R13 — Say "the shelf is the product" in the README, with the number
**ADDS.** §2.3: the tradition gives your collection 0–26% of the window at rest;
baz gives it 73–100%. That is the positioning, it is measured rather than
claimed, and no competitor can match it without rewriting their information
architecture.

### R14 — Persist the queue across quit
**ADDS — W2 is unimplemented.** Apple documents it; Feishin has `savePlayQueue`.
Cheap, band-C, and its absence is felt every launch. Do it silently: Feishin
needs seven settings and a *"Discard the current queue?"* dialog to manage queue
lifecycle because it syncs to a server. A single-user local player should never
prompt.

---

## 9. Walking every workflow through the recommended interface

The proposal is only as good as its worst workflow. Each row is the step count
after R1–R14, from a resting Library place. Anything clumsy is named.

| # | Workflow | Steps | Clumsy? |
|---|---|---|---|
| W1 | Put on an album | 1 (double-click) or 2 (click, `Play album`) | No — best in survey |
| W4 | Browse to decide | 0 | No |
| W12 | Back to what is playing | 1 (click the bar's now-playing block) — **only after R3** | **Today: impossible.** The proposal's biggest repair |
| W3 | Find a known thing | 1 (`/`) + typing | No |
| W9 | Inspect a release | 1 (click a tile) | No |
| W6 | See what is next | 1 (`Q` or the bar affordance) | No |
| W15 | Compare two releases | 1 per release, inspector follows selection | No above 940 px. **Below 940 px it breaks** — the shelf is gone, so comparison becomes back-and-forth. R9 fixes it |
| W2 | Resume | 0 after R14 | **Today: not implemented** |
| W5 | Shuffle | — | **Not in v0.1.** When it lands, it needs a home; the top bar has no subject and the bar has no room. Unresolved, and worth resolving before it is built |
| W7 | Change what is next | 2 (open popover, click a row) | Adequate for an album queue; R2 is the escape hatch when it stops being one |
| W8 | Add without losing | — | **No surface.** `SetQueue` replaces wholesale. Apple's stack-of-contexts is the model to copy when this lands |
| W13 | Facet a large library | — | **No surface.** The audit's §1.2 names this as a scope call; it remains one |
| W17 | Change density | 2 (Settings → Appearance) after R7 | Acceptable — band C, done once |
| W16 | Verify the chain | 0 (glance at the bar) / 3 (Settings → Playback → Signal path) | No — textbook progressive disclosure |
| W19 | Save the transient | — | Not in v0.1 |
| W20 | Scope the library | — | Not in v0.1 |
| W10 | Fix metadata | — | Not in v0.1 |
| W11 | Adjust sound | 2 (`Ctrl+,`, section) | No |
| W14 | Manage roots | 2 | No |
| W18 | Find holes | — | **No surface.** R10 |
| W21 | Recover the layout | — | **Does not exist, by design.** The tradition's workflow that baz correctly refuses to create |

| — | **The album ends** | 0 — silence | **Defect.** Longplay 1.0's behaviour, which Longplay reversed within one major version. R4 |

**Five defects the walk exposes, stated rather than dropped:**

1. **W12 is impossible today** and it is a band-A workflow. R3.
2. **The album boundary is a dead stop.** Not a workflow the user performs — one
   the product performs on them — and the one thing every album-first product
   has had to fix (§5.4). R4.
3. **W4 collapses at scale.** The shelf has no index, no jump-to-letter and no
   facets. At the 29-album fixture this is invisible; at Marta's 40,000 it is
   the product's main failure, and it is what Sonos users named first when it
   was taken from them. R8.
4. **W15 breaks below 940 px.** R9.
5. **W5 (shuffle) has no home in the proposed IA.** `VISION.md` pillar 4 makes
   steered shuffle a headline feature and v0.3 commits to it; `05-personas.md`
   §4 puts *"One toolbar shuffle button: 'Play my library'"* in the first-run
   sketch. The audit's four-kind model has no slot for it: the top bar has no
   subject, the bar reserves every pixel, and a place is too heavy. Prior art
   offers two shapes — Lollypop makes *"Random albums"* a **nav destination**,
   and Longplay makes shuffling the wall a **hold-to-confirm control on the wall
   itself**, on the reasoning that rearranging is destructive; the hold doubles
   as a scrubber through the collection. **This is the first thing the IA will
   be asked to absorb, and it should be answered before §2's model hardens** —
   not by this document, but by whoever owns increment 8.

---

## 10. Sources

Every URL below was fetched or examined during this study. Claims sourced from a
search snippet rather than a full page read are marked *(snippet)* at the point
of use in the text.

**Rendered first-hand** — see [`prior-art/`](prior-art/): fooyin 0.9.2,
Strawberry 1.2.18, Lollypop 1.4.45, on a private Xvfb in a throwaway container
(§0.1).

**Examined as images:**
[YouTube Music web player, Dec 2023](https://commons.wikimedia.org/wiki/File:Screenshot_of_YouTube_Music_web_player_(December_2023).png) ·
[Amberol 0.10.3](https://commons.wikimedia.org/wiki/File:Screenshot_of_Amberol_0.10.3.png)

**The bar-setters:**
[Roon — The Queue](https://help.roonlabs.com/portal/en/kb/articles/the-queue) ·
[Plex Labs — Introducing Plexamp](https://medium.com/plexlabs/introducing-plexamp-9493a658847a) ·
[Plex Labs — Plexamp v3](https://medium.com/plexlabs/plexamp-v3-9af3b10063b4) ·
[Plex — Play Queues](https://support.plex.tv/articles/202188298-play-queues/) ·
[python-plexapi — PlayQueue](https://python-plexapi.readthedocs.io/en/latest/modules/playqueue.html)

**Album-first:**
[Longplay](https://longplay.rocks/) ·
[Longplay FAQ](https://longplay.rocks/faq/) ·
[Longplay iOS guide](https://longplay.rocks/guide/ios/) ·
[Adrian Schönig — Introducing Longplay](https://adrian.schoenig.me/blog/2020/08/18/introducing-longplay/) ·
[Adrian Schönig — Introducing Longplay 2.0](https://adrian.schoenig.me/blog/2023/08/31/longplay-2.0/) ·
[Adrian Schönig — Essence of an App](https://adrian.schoenig.me/blog/2020/10/03/essence-of-an-app/) ·
[Jon Hicks on Longplay](https://hicks.design/journal/longplay) ·
[Six Colors — Longplay comes to the Mac](https://sixcolors.com/post/2025/07/next-album-up-longplay-comes-to-the-mac-at-last/) ·
[MacStories — Longplay 2.0](https://www.macstories.net/reviews/longplay-2-0-an-album-oriented-apple-music-player-with-loads-of-new-features/) ·
[Doppler docs](https://brushedtype.co/docs/doppler/) ·
[Doppler — add to queue](https://brushedtype.co/docs/doppler/add-to-queue/) ·
[MacStories — Doppler for Mac](https://www.macstories.net/reviews/doppler-for-mac-offers-an-excellent-album-and-artist-focused-listening-experience-for-your-owned-music-collection/) ·
[Albums](https://www.albumstheapp.com/) ·
[Albums 4.2 — queueing collections](https://albumstheapp.substack.com/p/music-app-stuff-7-albums-42) ·
[Albums 7.0 — the Mac app](https://albumstheapp.substack.com/p/albums-70-the-mac-app-a-glassy-ui) ·
[MacStories — Albums 4.0](https://www.macstories.net/reviews/albums-4-0-a-must-have-app-for-music-lovers/) ·
[Marvis Pro](https://appaddy.wixsite.com/marvis) ·
[Barrowclift — iOS music player showcase](https://barrowclift.me/articles/fourth-annual-ios-music-player-showcase/9)

**Sonos, 2024–2026:**
[Sonos — the redesigned app announcement](https://www.sonos.com/254658-sonos-unveils-completely-reimagined-sonos-app-bringing-services-content-and-system-controls-to-one-customizable-home-screen/) ·
[The Verge — the new app](https://www.theverge.com/2024/4/23/24137502/sonos-new-app-announced) ·
[Sonos — Update on the Sonos app (the apology and roadmap)](https://www.sonos.com/en-us/blog/update-on-the-sonos-app) ·
[Sonos community — personal music library features removed](https://en.community.sonos.com/controllers-and-music-services-229131/personal-music-library-features-removed-in-the-new-may-7-2024-app-version-6892421/index4.html) ·
[Jonathan Mosen — Sonos has broken accessibility for its blind users](https://mosen.org/sonos2024/) ·
[Ars — "It was the wrong decision": employees discuss the debacle](https://arstechnica.com/gadgets/2024/09/it-was-the-wrong-decision-employees-discuss-sonos-rushed-app-debacle/) ·
[Ars — CEO admits insufficient testing](https://arstechnica.com/gadgets/2024/10/sonos-ceo-admits-to-insufficient-app-testing-we-released-it-too-soon/) ·
[Ars — Spence steps down](https://arstechnica.com/gadgets/2025/01/sonos-ousts-executive-blamed-for-rushing-botched-app-update/) ·
[Roger Wong — When the music stopped](https://rogerwong.me/2025/02/when-the-music-stopped-inside-the-sonos-app-disaster) ·
[ecoustics — tabs return, opt-in, 2026](https://www.ecoustics.com/news/sonos-app-update-2026/)

**Heritage:**
[foobar2000 — Default User Interface](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:Components/Default_user_interface_(foo_ui_std)) ·
[foobar2000 — Layout Editing Mode](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:Layout_Editing_Mode) ·
[foobar2000 — Columns UI](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:Components/Columns_UI_(foo_ui_columns)) ·
[Columns UI docs (no layout chapter)](https://columns-ui.readthedocs.io/en/latest/) ·
[foobar2000 — Title Formatting Reference](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:Title_Formatting_Reference) ·
[foobar2000 — FAQ ("not possible since v0.9.5.3")](https://wiki.hydrogenaudio.org/index.php?title=Foobar2000:FAQ) ·
[foo_keep_queue](https://www.foobar2000.org/components/view/foo_keep_queue) ·
[Queue Viewer component](https://marc2k3.github.io/component/queue-viewer/) ·
[HA — "Any way to display the playback queue?"](https://hydrogenaud.io/index.php?topic=115857.0) ·
[HA — 2005 UI poll (Columns UI 84.8%)](https://hydrogenaud.io/index.php?topic=33270.0) ·
[HA — Default UI Gallery](https://hydrogenaud.io/index.php?topic=58574.0) ·
[foobar2000 changelog](https://www.foobar2000.org/changelog) ·
[Georgia-ReBORN issue #102, 64-bit compatibility](https://github.com/TT-ReBORN/Georgia-ReBORN/issues/102) ·
[HN — foobar2000 thread](https://news.ycombinator.com/item?id=40383935) ·
[AnandTech — Why do people have gripes about foobar2000?](https://forums.anandtech.com/threads/why-do-people-have-gripes-about-foobar2000.1614867/) ·
[MusicBee — Layout](https://musicbee.miraheze.org/wiki/Main/Layout) ·
[MusicBee — Theming](https://musicbee.miraheze.org/wiki/Main/Theming) ·
[MusicBee — Playing Tracks](https://musicbee.fandom.com/wiki/Playing_Tracks) ·
[MusicBee — Now Playing Preferences](https://musicbee.fandom.com/wiki/Now_Playing_Preferences) ·
[MusicBeeInterface.cs (the plugin API)](https://github.com/boroda74/TagTools/blob/master/Helpers/MusicBeeInterface.cs) ·
[Winamp](https://en.wikipedia.org/wiki/Winamp) ·
[Webamp source (the classic geometry)](https://github.com/captbaritone/webamp) ·
[Winamp Skin Museum](https://skins.webamp.org/)

**Mainstream:**
[Spotify desktop redesign, 2023](https://newsroom.spotify.com/2023-06-20/spotify-desktop-experience-redesign-your-library-now-playing-views-customize/) ·
[Spotify — Play Queue](https://support.spotify.com/us/article/play-queue/) ·
[Spotify — keyboard shortcuts](https://support.spotify.com/us/article/keyboard-shortcuts/) ·
[Windows Latest — Spotify moves the queue to the right side](https://www.windowslatest.com/2024/03/17/spotify-on-windows-11-gets-jam-and-moves-queue-to-the-right-side/) ·
[kroltan on Spotify's queue, HN](https://news.ycombinator.com/item?id=34259776) ·
[necubi on scroll latency, HN](https://news.ycombinator.com/item?id=24873575) ·
[Apple — Queue your songs](https://support.apple.com/guide/music/queue-your-songs-musb1e6d1c76/mac) ·
[Apple — Pin music](https://support.apple.com/guide/music/pin-music-mus953bc039a/1.6/mac/26) ·
[Apple — Search for music](https://support.apple.com/guide/music/search-for-music-mus896f20db7/mac) ·
[Apple Community — Tahoe transport legibility](https://discussions.apple.com/thread/256142581) ·
[Six Colors — macOS 26 Tahoe review](https://sixcolors.com/post/2025/09/macos-26-tahoe-review-power-under-glass/) ·
[Aadish Verma — how Apple Music's full-screen background works](https://www.aadishv.dev/music) ·
[9to5Mac — adaptive Apple Music design complaints](https://9to5mac.com/2026/03/31/the-new-adaptive-apple-music-design-draws-complaints-from-dark-mode-users/) ·
[TIDAL — Play Queue](https://support.tidal.com/hc/en-us/articles/360004182777-Play-Queue) ·
[TIDAL — HiRes FLAC](https://support.tidal.com/hc/en-us/articles/17412130162961-HiRes-FLAC-audio) ·
[TIDAL — enhanced credits](https://www.prnewswire.com/news-releases/tidal-launches-enhanced-credits-feature-to-spotlight-all-the-individuals-creating-music-300879572.html) ·
[PiunikaWeb — TIDAL 2026 redesign](https://piunikaweb.com/2026/05/06/tidal-ios-music-player-redesign-rolling-out/) ·
[9to5Google — YouTube Music web player redesign](https://9to5google.com/2024/03/04/youtube-music-web-player-redesign/) ·
[9to5Google — Now Playing gradient](https://9to5google.com/2023/12/12/youtube-music-now-playing-gradient/) ·
[Android Police — YouTube Music uploads](https://www.androidpolice.com/2020/06/09/hands-on-youtube-music-upload/) ·
[MusicTech — Adele and Spotify's shuffle default](https://musictech.com/news/industry/adele-gets-spotify-to-remove-shuffle-button-30-album-because-art-tells-a-story/)

**Open-source peers:**
[fooyin](https://github.com/fooyin/fooyin) ·
[fooyin — layout editing mode](https://docs.fooyin.org/en/latest/quick-start/layout-editing-mode.html) ·
[Tauon manual — gallery](https://tauonmusicbox.rocks/manual/gallery/) ·
[Tauon manual — interface](https://tauonmusicbox.rocks/manual/interface/) ·
[Feishin](https://github.com/jeffvli/feishin) ·
[Sonixd (archived)](https://github.com/jeffvli/sonixd) ·
[Supersonic](https://github.com/supersonic-app/supersonic) ·
[Strawberry](https://github.com/strawberrymusicplayer/strawberry) ·
[Strawberry forum — playlists vs queues](https://forum.strawberrymusicplayer.org/topic/361/understanding-playlists-vs-queues-vs-multiple-queues) ·
[Elisa](https://invent.kde.org/multimedia/elisa) ·
[Amberol](https://gitlab.gnome.org/World/amberol) ·
[Bassi — Amberol](https://www.bassi.io/articles/2022/05/25/amberol/) ·
[Museeks](https://github.com/martpie/museeks) ·
[Nuclear](https://github.com/nukeop/nuclear) ·
[Harmonoid](https://github.com/harmonoid/harmonoid) ·
[Quod Libet](https://github.com/quodlibet/quodlibet) ·
[DeaDBeeF](https://github.com/DeaDBeeF-Player/deadbeef)

**Collection browsers:**
[calibre manual — the GUI](https://manual.calibre-ebook.com/gui.html) ·
[GOG Galaxy](https://www.gog.com/galaxy) ·
[Steam library update](https://store.steampowered.com/libraryupdate) ·
[Steam Client Beta — grid size](https://steamcommunity.com/groups/SteamClientBeta/discussions/3/1627412105598183155/) ·
[PC Gamer — Steam collections](https://www.pcgamer.com/steam-finally-lets-you-take-control-of-your-game-collection-but-needs-more-options/) ·
[Julieanne Kost — Grid and Loupe in Lightroom Classic](https://jkost.com/blog/2024/06/working-in-grid-and-loupe-view-in-lightroom-classic.html) ·
[Apple Photos user guide](https://support.apple.com/en-us/guide/photos/pht56eafa987/mac) ·
[Android Police — Google Photos year view removal](https://www.androidpolice.com/2019/04/23/google-photos-has-removed-its-useful-and-compact-yearly-view/) ·
[Plex — Edit details](https://support.plex.tv/articles/201272763-edit-details/) ·
[Cloudscape — split view](https://cloudscape.design/patterns/resource-management/view/split-view/) ·
[Tom Amiri — improving the Criterion Channel's navigation](https://medium.com/@tomoamiri/improving-the-criterion-channels-navigation-design-4ec74d831f5c) ·
[Nick Schaden — Criterion and smart curation](https://www.nickschaden.com/2019/04/29/the-criterion-channel-and-the-appeal-of-smart-curation/)
