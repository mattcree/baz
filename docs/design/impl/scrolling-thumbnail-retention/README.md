# Scrolling thumbnail retention — implementation evidence

Date: 2026-08-14

## Failure reproduced

The prior test pinned one static 52-id target set and churned unrelated cache
entries. It never exercised the listener's transition. In production,
`ThumbCache::reconcile` moved a sleeve out of `resident` and into the 64-entry
`recent` LRU as soon as its row left the viewport. Dense traversal through more
than a few rows evicted it; returning then exposed the gradient while one of
two blocking workers decoded the prepared PNG again.

The new transition regression displays ids 1–18, traverses 44 more 18-id
viewports (810 displayed covers in total), then returns to ids 1–18. The old
implementation fails after the 64-entry boundary. The new implementation
finds every original handle immediately and schedules no reload.

## Lifecycle

A decoded thumbnail now has one of three homes:

- `resident`: named by the complete current wall/page/chrome target snapshot;
- `retained`: previously resident, and therefore actually presented during
  this process;
- `recent`: a bounded 64-entry LRU for work that completed after its target had
  already moved away.

Leaving a viewport moves `resident` to `retained`, never to `recent`. This is
session retention rather than an arbitrary larger LRU: it grows only when a
real visible target receives artwork and is bounded above by the indexed
collection. A density increase deliberately starts a new pixel-size generation
so Baz never stretches a tighter-density decode.

All three target categories now feed one ordered snapshot before the decode
queue is replaced. A smaller decode that finishes after density grows is put
back at the front without deleting the remaining visible work. The snapshot
also changes when the floating playlist panel opens, includes the silent Home
Continue album, and retains the existing Queue, saved/unsaved playlist,
Artist, lane, bottom-bar and collage nominations.

## Measurements

The owner library used for the acceptance run contained 8,602 tracks resolving
to 393 Baz albums. The prepared cache contained 672 PNGs using 157.7 MiB on
disk. A release build at Dense decoded 180 real covers into 27.3 MiB; the warm
prepared-cache completions were ordinarily 6–16 ms, with the observed long
tail reaching 34.1 ms. The app reported two jobs in flight at peak throughout.

Worst-case CPU RGBA if every album is visited and every cover is square at the
density ceiling:

| Collection | Dense, 200 px | Balanced, 288 px | Spacious, 320 px |
|---|---:|---:|---:|
| Owner, 393 albums | 60.0 MiB | 124.3 MiB | 153.5 MiB |
| Synthetic, 800 albums | 122.1 MiB | 253.1 MiB | 312.5 MiB |

Actual covers can be smaller or non-square, and the runtime log reports their
real RGBA byte total rather than this ceiling. The X11 acceptance process was
137,612 KiB RSS after startup, scan, playlist rendering and partial traversal;
that process figure includes the whole application, not only art.

Renderer residency does not duplicate the full session budget. iced 0.14's
wgpu raster cache records handles hit during the current renderer pass and
trims device atlas entries not hit after new entries land. The retained RGBA
handle lets a returning sleeve upload synchronously in the draw path, while
the device-side population continues to follow current visible targets. The
runtime `resident` count is therefore the conservative renderer-allocation
count; `retained` is CPU-only between visits.

## Verification

- `cargo test -p baz app::tests`: 51 passed.
- The 810-cover scroll-away/return regression passes.
- The stale-density retry preserves every other target queued behind it.
- Source regressions bind panel-open invalidation and Home Continue nomination
  to the complete snapshot.
- The actual release application launched in `baz-dev`, completed an unchanged
  scan of all configured local/NAS roots, and exercised Dense artwork loading
  with `BAZ_PERF_LOG=1`.

