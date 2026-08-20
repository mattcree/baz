# Smart shuffle — investigation, 2026-08-20

Item 57 asks whether baz's ordinary shuffle should gain flow. This records the
first investigation before changing a listener-visible traversal.

## What is already true

`baz_core::traversal::Traversal::Shuffled` is a seeded Fisher–Yates
permutation. It is a **bag**: every queue position is visited once, its order
is reproducible from the seed, and it changes no queue entry or queue order.
That is a good ordinary shuffle and must remain available. It is also the
semantics MPRIS can name with its boolean `Shuffle` property.

`baz-vibe` can make locally coherent sequences, but its existing `walk` is a
generated-playlist policy, not a shuffle policy. It intentionally limits an
artist to two selections and prefers fresh albums, and may return fewer than
its requested limit. Reusing it would silently drop queue entries — exactly the
failure a shuffle must not have.

The model/cache are entirely local. A flow arrangement may read **current
already-cached** feature rows; it must never start analysis, download a model,
or make a network request. A run with incomplete feature coverage must retain
ordinary shuffle rather than turning a transport preference into an unexpected
hour-long analysis task.

## Recommended shape

Keep ordinary **Shuffle** and add a distinct, opt-in **Flow** traversal. Flow
uses the same queue and still visits each queue position once; it changes only
the path through that queue. The queue page can therefore remain an honest
record of what the listener chose, while its existing planned-order reading can
show the actual next track.

Flow is available only when every path in the new run has a current local
feature vector. If it is not, the action states that it needs locally analysed
music and offers no surprise work. It does not replace ordinary Shuffle, does
not persist as MPRIS `Shuffle`, and a client writing MPRIS `Shuffle = true`
continues to select ordinary random shuffle.

This preserves three standing rules:

1. no track is added, removed, or duplicated;
2. a listener can still request an unbiased random pass; and
3. baz's local/offline promise is unchanged.

## Implementation boundary

The engine currently derives a plan from `Traversal`, which is `Copy` and
holds only a random seed. Flow needs an explicit permutation calculated by the
shell from the cache, then carried by a new non-`Copy` traversal variant. The
engine must validate it is a permutation of `0..queue.len()` before accepting
it; invalid or stale plans must be refused rather than improvised.

The arranging policy is a new bounded greedy walk, not `baz_vibe::walk`:

- first track: seeded random choice;
- thereafter: choose among a deterministic bounded candidate sample by sonic
  distance, with a soft adjacent-artist penalty;
- never exclude an entry merely because its artist or album has appeared;
- append the remaining entries in a deterministic order if the bounded search
  reaches its stated work budget.

That bound is essential. A naive all-remaining-neighbours scan is quadratic
and is not acceptable for a 9,000-track All songs run. The implementation must
benchmark its actual 9,000-track work and add a regression budget before the
control is exposed.

## Work still required

1. Add a cache-only preparation path that joins queue paths to current local
   feature rows without starting analysis.
2. Implement and test the total permutation/coverage algorithm and its bounded
   work budget over a synthetic 9,000-track set.
3. Carry and validate a planned traversal across the shell/core protocol,
   retaining `force_next`, repeat, queue edits and current-track continuity.
4. Add the Flow control and its unavailable explanation, then exercise it with
   a real analysed library.

