# The 2026-08-14 backlog pass

Eleven of the twelve asks the owner logged on 2026-08-14 shipped together,
because eight of them are the same twenty-four hours of work coming back: the
control pass (WORK.md item 38) changed `ICON_PX`, `TRANSPORT_HIT` and
`STEPPER_HIT`, and every surface those three feed was re-derived except the
ones below.

## What the frames show

`01-library.png`, `02-playing.png` — 1400 × 1000, isolated Xvfb, the 25-album
silent fixture. `04-debug.png` — Settings → Debug. `hist2.png` — the app bar's
leading cluster at 5×, point-filtered.

- **The application's mark and the four lane glyphs stand on one centre**, 32
  px from the window's leading edge, and are now the same 32 px square. The
  assertion that holds them there is back in
  `the_lane_has_two_widths_and_a_floor_that_chooses`.
- **The `RECENT` heading is gone** and the head's destination tiles and the
  list below the rule share one 48 px pitch.
- **Back and forward are browser arrows again**, pointing the right ways
  (`hist2.png`).
- **The Favourites heart is in the bottom bar**, beside the sounding track's
  name, inert here because nothing is sounding and nothing is in the library
  to heart.
- **Settings → Debug reports this process's own resident memory and CPU.**

## The one that was not what it looked like

The owner's *"the back button icon is wrong and so is the forward"* had been
answered once already, by redrawing the two outlines. It came back because
**the outlines were never what was on screen**: `Glyph::ALL` and
`Glyph::index` are two hand-written orderings of the same list, `VisualFacts`
was appended to one before the history pair and numbered after them in the
other, and the sheet therefore handed out four wrong sprites —

| asked for | drew |
|---|---|
| `HistoryBack` | the facts mark (three bars) |
| `HistoryForward` | the back arrow |
| `Bell` | the forward arrow |
| `VisualFacts` | the bell |

Nothing could have caught it. `every_glyph_rasterizes_to_the_same_square`
walks `ALL`, and `the_sheet_hands_out_one_stable_handle_per_glyph` checks a
handle is stable — a permutation is invisible to both, because every sprite
exists, every sprite is the right size and every glyph gets *a* stable handle.
Only the pairing was wrong, and the pairing was the one thing neither named.

It is a **`const` assertion** now, at module scope in `icon.rs`, not a test:
the two lists are a duplication the type system cannot remove (a match arm per
variant is what makes adding a glyph a compile *error* rather than a silent
gap), so the check belongs where it cannot be run past. It was verified to
fire by reintroducing the swap.

The outlines were separately wrong and are separately fixed. The old form was
one self-intersecting nine-vertex polygon whose overlap **cancelled** under
the even-odd cast, leaving a hollow head whose "stroke" was the sliver between
the triangle's edge and the shaft's diagonal — tapering from a hairline at the
back corners to six times that near the tip. There was no stroke weight to
re-proportion for `ICON_PX` 20 because there was no stroke. They are three
plain outlines now — two 45° arms and a shaft, all at the set's 0.145, which
is `OPEN`'s and `ARROW_UP`'s weight.

## The app bar was 156 px wider than it said

*"The window controls disappear when we make the window narrow"* was not the
10 px of slack `APP_BAR_LINE` used to leave against a 712 px floor. The sum
was not the bar: the Back/Forward pair (84 + a 16 px seam) and the health bell
(40 + a 16 px seam) both shipped into the drawn row on 2026-08-13 and neither
ever entered the budget. The real line was 858, so the three buttons — the
row's last child — went off the trailing edge **146 px before the floor**.

Every test that could have caught it recomputed the constant's own expression,
so the arithmetic agreed with itself all the way down and never met the
geometry. `the_app_bar_holds_its_tenants_at_the_windows_own_floor` now **walks
the tenants of `app_bar::view`'s own `row!`**, pinned to that source, and
derives the line from the walk.

`APP_BAR_LINE` 702 → **850**; `WINDOW_FLOOR_W` 712 → **860**. The floor did not
move because the bar grew — it moved because the bar was finally measured. The
alternative was letting the search well yield as the window narrowed, which
would put the one app-wide control on a measure that changes underneath the
query in it; ADR-0040 §4 makes the buttons unconditional, so they are not what
may yield.

## Measuring baz with baz

Settings → Debug's new readout was pointed at the app it lives in on its first
run and reported **99.9 % of one core, idle on the Library**. Independently
confirmed by sampling `/proc/PID/stat` from outside: 999 jiffies over 10 s.

**It is the harness, not the product.** That run forced `ICED_BACKEND=tiny-skia`
— the software path, which has no vertical blank to block on and spins. The
same measurement on the shipped default renderer is **4 jiffies over 10 s**,
4 % of one core.

Worth writing down because `docs/DEVELOPMENT.md`'s headless recipe and
`docs/screenshots/capture.sh` both reach for tiny-skia, so the next person to
measure idle cost the documented way will find a phantom.

## Isolation

Every run took all six XDG redirections. `[mpris] no session bus` is in each
log as the receipt. The fixture is `mkfixture.sh`'s silent FLACs; the scratch
`HOME` routes ALSA's default PCM to `null`.

## The bell: three faults in one 20 px square

`bell.png` is the health indicator at 6×, after. Before, it was a plain
coloured disc — and had been for as long as it existed.

1. **The dot painted the whole glyph box.** `views::status` carried
   `theme::status_dot` *and* `align_right(Length::Fill)` /
   `align_bottom(Length::Fill)` on one container. Those two calls set that
   container's bounds to `Fill`; a container paints its **own** bounds; the
   999 px corner radius therefore became a disc exactly the size of the glyph
   underneath it.
2. **The glyph underneath was the forward arrow**, from the `Glyph::ALL` /
   `Glyph::index` disagreement above.
3. **The real `BELL` outlines were a blob too** — 0.56 wide by 0.60 tall with
   near-vertical sides, and a "base" flush with the body rather than a rim, so
   there was no mouth. The doc comment's *"at the shared icon stroke"* had
   never been true of them.

Each fault hid the next, which is the part worth keeping: two of the three had
shipped for months behind the third, and the only reason any surfaced is that
fixing the outermost exposed the one below it.

The bell is a silhouette on `HOME`'s precedent rather than a stroke. The
sheet's stroke rule is about **open angles** — `OPEN` and the history arrows
are strokes so they cannot read as `PLAY`'s solid mass — and a bell has no such
twin. What a bell needs is a profile, and a profile drawn at 0.145 with four
parts is a tangle at 20 px where a silhouette is instant. So the ratio does the
work: a 0.30 dome flaring into a 0.68 mouth, where the old shape's were 0.34
and 0.56.
