# Rootless legacy-row removal

Schema v8 gave every newly scanned row a library root and launch adoption
claimed compatible legacy rows. The remaining population—paths under none of
the configured roots—was visible as a count in Settings but no root scan could
ever prove or prune it.

`Library::unrooted_paths` is the read-only projection behind both count and
preview. Settings gives it a separate `Outside held folders` block because a
rootless row is not necessarily a missing file. The first press reveals every
path and the exact index-only consequence; the second is `Remove from index`,
beside inert `Keep`. Scans cannot trigger this route.

Confirmation reuses `Library::forget_paths`, including its transaction and v9
first-seen tombstones. It then rebuilds shelves and visible artwork requests.
The action never deletes audio, edits playlist files or listening history, or
changes the live run. A later scan after adding the folder back can restore the
rows with their original ADDED dates.

The migration regression now also checks the exact stable rootless path list,
and the all-feature Baz check covers the complete message/view wiring.
