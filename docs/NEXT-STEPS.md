# baz — Next Steps

> Concrete, ordered, with acceptance criteria. Standards in `ENGINEERING.md`;
> vision in `VISION.md`; deliberate deferrals in `BACKLOG.md`; what actually
> landed in `CHANGELOG.md`. Updated 2026-08-10.
>
> **Status**: Phases 0–3 ✅. baz scans several folders, shows the collection,
> plays it gaplessly and bit-perfectly, searches by song and by record, edits
> its queue, keeps playlists as files you own, and undoes what it did. It has
> a resident sidebar, a Home, a Now playing place, and one press to sound from
> the wall. 33 ADRs, 13 design studies, 1047 tests, CI green on three
> platforms. **Nothing has been released.**

## Where the work actually stands

The plan this file used to hold is spent. What replaced it, and where each
chapter's reasoning lives:

| Chapter | State | Reasoning |
|---|---|---|
| Engine: gapless, seek, bit-perfect, ReplayGain, volume, exclusive output | shipped | ADR-0004/0007/0009/0011/0012/0013/0015 |
| Library: several roots, refresh, unavailable-folder honesty, picker | shipped | ADR-0022, ADR-0025 |
| Interface: places, motion, group keys, search ranking, the index rail | shipped | ADR-0016–0022, design 01–07 |
| Playback model, named and stated | shipped | ADR-0023, design 08–09 |
| Playlists: `.m3u8` files, page, panel, sleeves, one transfer gesture | shipped | ADR-0024, design 08–09 |
| Controls, iconography, the strip budget | shipped | ADR-0026, design 10 |
| Forgiveness: undo, trash-backed delete | shipped | ADR-0027, design 11 P2 |
| Direct manipulation: drag to reorder and to add | shipped | design 09 §13 step 8, design 11 P5 |
| Density's visible control | shipped | ADR-0028, design 11 P8 |
| One press to sound from the wall: the hover options | shipped | the owner's design, 2026-08-09 |
| The returns lane, `Home`, `Now playing`, search in the lane | shipped | ADR-0030, design 13 |
| The `CONTINUE` band — the question asked in the silence | shipped | ADR-0030's third amendment |
| The ambient Now playing: field, meter, spectrum, feed, toggles | **designed, unbuilt** | ADR-0029, design 12 |

## Next, in order

### 1. Cut v0.1 — the first tag

**The largest gap in the project is that none of this is installable.** The
README says pre-alpha and the releases page is empty; `RELEASING.md` and
`INSTALL.md` both describe a process nobody has run. Everything below is
smaller than this.

1. ~~**An application icon.**~~ **Done.** `packaging/icons/` — an SVG master, a
   second source for the three smallest sizes, and the hicolor PNG ladder;
   `Icon=io.github.mattcree.baz` in the desktop entry; installed by the
   Flatpak and carried in the Linux tarball. `packaging/icons/README.md` says
   what the mark is and why. The binary still sets no window icon: winit 0.30
   supports that on Windows and X11 only, and the reasoning and the patch are
   in that README.
2. ~~**Dry-run the release**~~ **Done locally, not in CI.** `RELEASING.md` now
   carries the rehearsal it never had — every gate, the release build, the
   staging and the checksum step, all run on the maintainer's machine, with
   the corrections that turned up. What is still unrehearsed is the part only
   GitHub can run: `workflow_dispatch` on the release workflow, which nobody
   has fired.
3. ~~**Verify the Flatpak actually builds**~~ **Done.** It does now. It did
   not before: `cargo-sources.json` had seven crates unpacked without their
   `.cargo-checksum.json` and cargo refused the whole build. Regenerated,
   built, installed, run headless. `check-cargo-sources.py` now fails on that
   shape, so CI catches the next one.
4. **What is left before a tag.** A screenshot committed for the metainfo
   (Flathub rejects a submission whose `<image>` does not resolve — GitHub
   releases do not care); the version edit and metainfo release entry from
   `RELEASING.md` steps 2–5; a `workflow_dispatch` dry run; then the tag.

**Accept**: a stranger on Linux, Windows or macOS can download or
`flatpak install` baz, point it at a folder, and hear music — without
building it.

### 2. Close the honest gaps in what already ships

- **Opus** — refused four ways with the reversal conditions written down
  (`BACKLOG.md`); re-check whether Symphonia has merged a decoder.
- **Vorbis seek loses one lapped block** (23.2 ms, measured and pinned).
- **`Locate…` for missing playlist entries** — ADR-0024 §3 specifies the
  repair surface; the page only counts and shows the broken path today.
- **Shortcut discovery** — the bindings live in the README and nowhere a
  running baz can show them.

### 3. Build the ambient Now playing

Designed in full and not started: the cover-derived field, the R128 momentary
meter, the **spectrum analyser** (whose FFT costs no new crate — `rubato`
already pulls `realfft` into every build), the local facts feed, and the four
toggles. ADR-0029 and `docs/design/12-now-playing-and-kiosk.md` carry the plan:
nine steps, or **1 → 2 → 6 → 8** to reach the bars in four. The owner's rule
governs it — *"ambient motion is fine as long as the performance remains top
tier"* — so §7.4's harness and its four thresholds are the gate, and the
measurements have never been taken because the feature does not exist yet.

### 4. Three decisions waiting on the owner

- **The borderless window.** Wayland already draws that title bar inside baz's
  own process, so turning it off is one field — but iced 0.13 exposes no
  edge-drag resize anywhere in `window::Action`, so going borderless today
  loses pointer resizing. The route is a ~30-line upstream-shaped iced patch,
  which means a forked dependency.
- **Shuffle as a toggle.** It has a mechanical problem, not just a
  philosophical one: turning it *off* has nothing to restore, since playing a
  playlist copies it and decouples. Reversing act → mode would also un-block
  the crossed-arrows glyph that doc 10 refuses only because shuffle is an act.
- **Whether `Pull` goes.** Self-contained, sends no engine command, and its
  removal would answer design 11's P9 (*"explain it or rename it"*) a third
  way. If it, `Shuffle` and `Play all` all went, the strip's acts lane drops to
  zero and the two-line split loses its reason to exist.

### 5. The chapters not yet begun

- **Steered shuffle / generated playlists.** The ground rules are already
  law (ADR-0024 §7, design 09 S10): an ordinary editable file, asked for by
  a person, no hidden pool. The signal — bliss-rs or equivalent local
  analysis — is unbuilt. `VISION.md` stages this; nothing about it is
  urgent.
- **Enrichment, scrobbling, tagging** — the paid-parity extensions, each
  individually opt-in, none prioritized over the core.

## Two standing constraints that outlive this file

- **Accessibility is blocked at the toolkit**, not at baz. iced 0.13
  publishes no accessibility tree; the README says so before install, and
  ADR-0017 §4 refuses to inherit the gap quietly. If AccessKit lands in
  iced, baz's side is small — every control is already a labelled widget
  rather than a positional mark. **Revisit at every iced upgrade.**
- **iced 0.14 exists and is not taken.** No feature needs it today; it is
  worth an ADR when one does, or when AccessKit arrives.

## Standing rules while executing

- CI is the guide: main goes red, main gets fixed, before anything else.
- Every stack-level choice becomes a short ADR at the moment it is made.
- `REFUSALS.md` binds contributors and agents, **not the owner**: his decision
  is sufficient, and an entry he reverses is rewritten to record it. For
  everyone else the editing rule stands — removing an entry needs an argument
  that beats it.
