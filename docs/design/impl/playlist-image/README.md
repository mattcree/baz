# A playlist wears a picture the listener chose — item 52

The owner, 2026-08-15: *"lets allow setting an image/removing the image for a
playlist."*

## The decision the backlog left open

A playlist's sleeve is a **collage of quotations** from the records it holds
(ADR-0024 §A1) — generated, never authored, so it cannot disagree with the
tiles of the records it quotes. A chosen picture is a second kind of sleeve,
and the backlog named three questions rather than a design.

**Where the bytes live: beside the list.** `<name>.png` next to
`<name>.m3u8`, in the listener's own playlists folder. The alternative — a row
in baz's database — would make a picture the listener chose invisible to every
other program, absent from a copy of the playlists folder, and lost on a
reinstall. For a product whose first promise is *your files are the truth*,
that is not a real option. The cost is stated rather than discovered: the
picture appears in their folder, where they can replace or delete it
themselves, and where a **rename has to carry it** and a delete has to take it
along. Both do.

The bytes are **copied, not referenced**. A path into somebody's pictures
folder breaks the day they tidy it, and would be missing from the one portable
artefact baz promises. The copy keeps the source's own extension, so nothing is
re-encoded and no image *encoder* enters the dependency tree —
`IMAGE_EXTENSIONS` is `jpg`, `jpeg`, `png`, `webp`, and anything else is
refused where the rule lives (`Folder::set_image`) rather than in the dialog's
filter.

**What happens to the collage: it comes back.** Removing the picture is not
"blank the tile" — the collage is what a playlist's sleeve *is* when nobody has
said otherwise. Removal goes to the **platform trash**, like deleting a list
does (doc 11 §5 P2): it is the listener's own picture in the listener's own
folder.

**What the picker is: the platform's file dialog**, `rfd`, on the blocking pool
— the second thing in the product to open one, after choosing a music folder,
and it follows `pick_folder`'s rule exactly so the event loop never waits on a
human deciding.

## What it draws

`views::playlist_sleeve_of(shelf, id, art, name, edge)` is the one function the
wall's tiles, the returns lane and the panel's rows call, and
`playlist_sleeve_authored` is the page's — a list cannot look like two
different objects in two surfaces, which is the collage's own argument.

**Cover, not fit.** A sleeve is a square hole everywhere baz draws one, so a
listener's 3:2 photograph is cropped to the square from its centre rather than
letterboxed with the room's ground showing through the middle of a shelf of
covers. It is never enlarged past its own pixels: `art::load_picture` is
downscale-only, like every other tier in that module.

One decode at `art::THUMB_PX` (320) serves all three surfaces, because 320 is
exactly `theme::ART_MAX` — the largest a playlist sleeve is ever drawn. The
decode rides with the collage requests (`request_playlist_art`), is keyed by
playlist id, and is dropped when the picture changes: the *path* can be the
same and the bytes different.

While a decode is in flight, or where the file cannot be read at all, the list
draws its collage — the same honest reading a record's tile gives art it cannot
decode.

## The acts, and the thing they broke

The page gained `Set image…` / `Change image…` and, only where there is one to
remove, `Remove image` — absent rather than disabled, the house rule. That made
**four** acts under a `Rename · Delete` row that had always been two, and the
aside is `ALBUM_ASIDE_W` wide and does not grow: the fourth word drew half off
the edge, which the first frame of this capture caught.

`page::view` now lays acts **two to a line**. Pairs rather than a measured wrap
because a `Row` cannot ask how wide its children want to be — and because it
leaves every page with one or two acts, which is every other page in the
product, pixel-identical.

## The frames

`capture.sh`, headless at 1280 × 860 with all six XDG redirections; the
`[mpris] no session bus` line is the receipt.

| | |
|---|---|
| `01-wall-one-picture-one-collage` | The Playlists wall: `Road Trip` under `R` wearing its picture, `Sunday Morning` under `S` wearing its collage, and the lane showing both at 40 px. |
| `02-page-with-its-acts` | The list's own page: the picture as the page's sleeve, and `Rename · Delete` over `Change image… · Remove image`. |

**What the capture can and cannot photograph.** Setting a picture opens the
platform's file dialog, which on Linux is a D-Bus portal a headless run has no
claim on — `rfd` returns `None` at once and the act is a dismissal. So the run
seeds the sibling file the dialog would have produced, and photographs
everything downstream of it, which is the half that could quietly not work. The
copy itself is covered by two tests instead:
`baz_core::playlist::tests::a_playlist_wears_one_authored_sleeve_and_keeps_it_through_a_rename`
(the file lands, one file per list however often it changes, and a rename
carries it) and
`playlists::tests::a_chosen_picture_reaches_the_row_and_the_open_page` (the row
every tile reads, and the page whose acts depend on it).

Neither test asserts *removal*, and that is stated rather than hidden: removal
spends the platform trash, which a test process has no claim on either.
