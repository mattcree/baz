# Enqueue Next and End

Search track rows now distinguish the two live-run insertion intentions only
when the distinction exists:

- an existing run: `Play | Next | End`;
- no run: `Play | Enqueue`;
- a saved-playlist page: `Play | Add to playlist`.

The same row buttons and Left/Right action axis select those acts. Neither
insertion sends Play or replaces the run.

## Stable repeated Next

Inserting every press at `cursor + 1` reverses a sequence. `NextAnchor` records
the current track sequence, expected queue length and next insertion slot, so
pressing A then B produces `current, A, B`. A track boundary or mismatched edit
starts a fresh anchor after the newly confirmed cursor. `queue_edit::inserted`
performs one absolute ordered splice and rejects stale/out-of-range slots.

## Shuffle remains honest

A plain `UpdateQueue` re-derives a shuffled permutation, so putting a row after
the cursor in list order alone cannot promise it will play next. The new
`UpdateQueueNext { paths, next }` retains the protocol's absolute whole-list
shape and additionally names one absolute successor. Engine and front-end call
the same `traversal::force_next`: it moves that position immediately after the
current traversal slot (or to the top while stopped), leaving every other
position in its prior relative order. The explicit choice wins once; shuffle
continues afterward.

Tests cover collapsed/expanded keyboard axes, repeated insertion order,
absolute splices, stable command JSON and forced successors over a shuffled
pass. Isolated 1280 × 860 renders showed `Play | Enqueue` before a run and
`Play | Next | End` after playback began.
