# Visible artwork residency — all-consumer audit

Backlog item 20, 2026-08-14.

> Follow-up, 2026-08-14: item 30 reopens the broader listener-visible promise.
> This pass proved residency inside one static target set; it did not simulate
> scrolling more than 64 covers away and returning, so it did not prevent a
> previously displayed handle from being evicted and visibly reacquired.

The cache design was already the right small design: current wall, page and
resident-chrome targets form one un-evictable `HashMap`; handles leaving every
current set return to the existing 64-entry recent LRU. Decode work has one
two-job priority scheduler. This pass adds no cache tier and no speculative
prefetch.

The defect was incomplete nomination:

| Consumer | Existing request | Defect | Repair |
|---|---|---|---|
| Library wall | measured visible/overscan range | none | unchanged |
| Album/Now playing hero | current subject | none | unchanged |
| Artist page | all records the non-virtual page draws | none | unchanged |
| Playlists overview/page | visible tiles, header collage and visible rows | none | unchanged |
| Queue page | header and visible rows | later chrome pass cleared `page` in the same update | Queue retains its page pin set |
| Home | Recently added | always-visible All songs collage omitted | `home_art` returns both sets |
| Returns lane/bottom bar | measured lane window and sounding album | none | unchanged |
| Floating playlist panel | no nomination | collages depended on unrelated cache luck | panel quotations and All songs join chrome targets |

`every_visible_consumer_survives_eight_hundred_album_churn` pins simultaneous
wall, chrome and page sets, inserts 748 further album handles, verifies every
visible handle remains, and verifies the off-screen tier is still exactly 64.
The smaller tests retain the original eviction reproduction and prove that a
handle returns to the bounded LRU immediately after its final surface leaves.

The logic is renderer- and platform-independent and is ordinary Rust in the
Linux and Windows CI targets. The prior release GUI stress (80-record Artist
page, about 25.3 MiB resident) and isolated multi-page fixture remain the visual
checks; this item closes the target-supply holes that made larger Windows
libraries expose the problem more often.
