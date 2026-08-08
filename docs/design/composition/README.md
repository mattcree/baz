# composition — the frames, the overlays and the rulers

Everything [`../06-composition-audit.md`](../06-composition-audit.md) measures.

- **[`shots/`](shots/)** — 32 unmodified frames from the real release binary,
  `--features device-output`, on a private `Xvfb` with all six XDG redirections
  from [`../../DEVELOPMENT.md`](../../DEVELOPMENT.md#headless-ui-verification).
  16 states × **1280 × 860** and **1920 × 1080**. Every run logged
  `[mpris] no session bus`; the fixture is digitally silent FLAC and the scratch
  `HOME` routes ALSA's default PCM to `null`.
- **`0*.png`, `10-*.png`** — the same frames with the rulers drawn on. Blue is
  an edge two or more elements share, red is an edge nothing else shares, amber
  is a centre or rhythm line.
- **[`tools/`](tools/)** — the rulers themselves. `ruler.py` is a
  standard-library PNG decoder plus the measurements (ink boxes,
  contrast-weighted centroids, coverage, lattice fitting); `census2`–`census5`
  are the passes; `mkfixture.sh` builds the library and `capture.sh` drives the
  binary. `python3 tools/census2.py 1280x860` reproduces §1 of the audit.

| overlay | what it shows |
|---|---|
| `01-wall-edges-1280.png` | the wall's 16 x-edges; the chrome's 16 px gutter against the works' 40 |
| `02-bar-centrelines-1280.png` | the bottom bar's four mark-lines, none of them the bar's own mid-line |
| `03-topbar-baselines-1280.png` | `Settings` 8 px above the counts it shares a row with |
| `04-inspector-edges-1280.png` | 8 x-edges in a 340 px column, 5 of them singletons |
| `05-settings-edges-1280.png` | a content column that ends at x 878 at every window width |
| `06-queue-edges-1280.png` | four left edges in a 358 px popover |
| `07-first-run-centring-1280.png` | a block centred to the pixel whose ink is 93 px off centre |
| `08-wall-edges-1920.png` | the hang at six columns; the chrome's edges unmoved |
| `09-inspector-edges-1920.png` | the inspector, identical at 1920 |
| `10-bar-centrelines-1920.png` | the same four lines at the larger window |
