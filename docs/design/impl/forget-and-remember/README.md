# Forget, and remember — ADR-0042's evidence

What `docs/WORK.md` beta blockers 2 and 3 turned out to be: one question,
*what should baz remember about music it no longer holds?*, and one answer —
**when it first saw it, and nothing else, because nothing else is
unrecoverable**. [ADR-0042](../../../adr/0042-what-baz-remembers.md) has the
argument. This directory has the proof.

## The run

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=<scratch> \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev env BIN=<scratch>/release/baz \
  docs/design/impl/forget-and-remember/harness.sh
```

Headless (Xvfb), isolated six ways, silent fixture, `BAZ_DEVICE_TESTS` unset.
The run prints its `[mpris] no session bus` receipt.

## What it proves

**The value from before, not merely a value.** A fixture scanned and re-scanned
inside one afternoon reads `This evening` at both ends and proves nothing. So
the 25-album fixture is *aged* first — its rows' `first_seen_ns` written
straight into the scratch database at four years, one year and three months old,
which is the one fact in the schema no press can reach (ADR-0019 §5) and
therefore the one thing a harness is entitled to arrange. Everything the feature
does happens through the product, by presses.

Then the whole round trip is driven the way a listener drives it: the gear, the
Settings place's `Library`, the folder's own `Remove`, the confirming `Forget`,
the add-a-folder well, its `Add`.

| | |
|---|---|
| `01-added-before.png` | the wall grouped by ADDED — `3 MONTHS AGO`, `1 YEAR AGO`, and `4 years ago` down the index rail |
| `02-settings-library.png` | the folder, its 206 tracks, its last scan |
| `03-remove-armed.png` | **the sentence**: *Forget 206 tracks? The files stay on disk; baz stops holding them but remembers when they arrived.* |
| `04-folder-forgotten.png` | `No folders yet.` — the act is done, 206 tombstones written |
| `05-wall-empty.png` | what that means on the wall: `Nothing here yet` |
| `06-adding-it-back.png` | the same path typed back into the well |
| `07-settings-rescanned.png` | 206 tracks again, and **zero** tombstones — every one consumed by the return |
| `08-added-after.png` | the wall |
| `09-remove-armed-960.png` | the same sentence at the narrowest width the place has been photographed at — one line, no wrap, no clip |

Two assertions, and the run fails loudly on either:

1. **`SELECT first_seen_ns, count(*) FROM tracks GROUP BY first_seen_ns`** is
   byte-identical before the forget and after the re-add — the same three
   nanosecond timestamps against the same three counts, not three timestamps of
   the same *age*.
2. **`01-added-before.png` and `08-added-after.png` differ in `0` pixels.**
   The wall the listener gets back is the wall they had, and that is a number
   rather than a recollection of an earlier frame.

## What it does not photograph, and where that lives instead

The **record-scale** half — `Library::forget_paths`, and the case it exists to
insure (a listener asserting a record is gone while the share is merely
unmounted, and the remount costing them nothing) — has no visible control yet;
ADR-0042 §8 says why and draws the one that is missing. It is proved at the
library level instead, against real files and real scans, in
`crates/baz-core/tests/index.rs`:

- `forgetting_a_record_that_was_only_unmounted_costs_nothing_when_it_returns`
- `forgetting_a_record_keeps_when_it_arrived_and_the_files_coming_back_restores_it`
- `forgetting_a_root_and_forgetting_its_paths_leave_the_same_memory`
- `a_scan_confirmed_removal_remembers_nothing_and_a_listeners_does`
- `a_path_forgotten_many_times_is_remembered_once_and_never_while_held`
- `forgetting_and_restoring_a_folder_leaves_the_play_ledger_alone`
- `a_v8_database_migrates_in_place_without_losing_anything`

## What it cost, measured

`scan/launch_cold_10k` — the benchmark ADR-0010 recorded 83.4 ms for — measures
**81.0 ms** on the development host with this change in. The scan's per-file
path gains one hash probe of a map that is empty in the ordinary library, and it
does not appear above the difference between two runs.
