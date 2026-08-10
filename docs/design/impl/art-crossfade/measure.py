#!/usr/bin/env python3
"""Read `capture.sh`'s film back and say what the crossfade actually did.

A still frame cannot prove motion, and a *pair* of stills cannot tell a
crossfade from a cut.  So this reads **every frame** of the 60 fps film and
answers four questions with numbers rather than with an eye:

1. **Did the track change three times?**  The placard's title is compared with
   the frame before it; each frame whose title pixels moved is a track change.
   Without this the negative case would be vacuous — *nothing moved* is not
   evidence if the key press never arrived.
2. **Did the sleeve travel, or jump?**  Each frame's mean sleeve colour is
   projected onto the line between the two records' settled colours, giving
   `t`: 0 at the outgoing record, 1 at the incoming one.  A cut spends **no**
   frame strictly between the two; a 200 ms dissolve spends about twelve.
3. **Did the field travel with it?**  The same projection over a strip of pure
   field at the window's right margin.  One number drives the cover and the
   room in the shipped code, so the two ladders have to be *the same ladder*;
   a divergence would be the seam the owner had removed, back in time.
4. **Did nothing move where nothing should?**  Two of the three track changes
   stay inside one record.  Every frame around them must read `t = 1` on both
   probes and show **no sleeve pixels moving at all**: consecutive tracks on
   one record share a cover.

# The probes are named, not found

A probe that hunted for the sleeve could find whichever record's sleeve it
preferred.  These are rectangles of the 1280 x 860 composition:

* **sleeve** — the artwork is drawn at x 320..776, y 88..544 (`HANG` 40 off a
  body that starts at the returns lane's 280, `RUN_MEASURE` taken on the
  right).  The probe is the inner 400 x 400, so an antialiased edge cannot vote.
* **field** — x 1244..1276 over y 100..700: the `HANG` of margin outboard of
  the run column.  No type, no artwork, no scrollbar; one wash.
* **title** — x 320..776, y 578..628, the placard's track title.  Used as a
  *difference* and never as a mean: the box is mostly field, and a mean over it
  cannot see one word replaced by another.  The first cut of this script made
  that mistake and reported a key press that had in fact arrived as missing.

Usage: `measure.py FRAMES_DIR OUT_DIR BUILD`
"""

import pathlib
import sys

import numpy as np
from PIL import Image

SLEEVE = (348, 116, 400, 400)  # x, y, w, h
FIELD = (1244, 100, 32, 600)
TITLE = (320, 578, 456, 50)
ART = (320, 88, 456, 456)  # the whole sleeve, for the figures
WHOLE = (0, 0, 1280, 860)  # every pixel: how often the window is repainted
# A sliver just inside the **left edge of the full-size sleeve**. The artwork
# is drawn at `record_edge`'s answer and centred in its column, so this strip is
# artwork while the hero is being drawn and *field* while a 320 px thumbnail is
# standing in for it — which is how the size of the sleeve is read off a frame
# without hunting for its edges. It is the wart the hold was built to remove:
# before this branch, a record change cut to the thumbnail, at 320 px on a room
# with no field, and popped to 456 px when the decode landed.
#
# It is at the sleeve's **right** edge and not its left: `record_column` is
# left-hung and shrink-wrapped, so a 320 px sleeve keeps the column's left edge
# and gives up the right. A probe on the left would sit inside the small sleeve
# and report a full box — which it did, until this frame set showed otherwise.
EDGE = (758, 300, 14, 100)

# What share of a probe's pixels must move between two frames before the probe
# counts as having changed.  The film is lossless (ffv1), so the only noise is
# a torn grab; 0.5 % of a box is far above that and far below a title being
# replaced or a sleeve being redrawn.
CHANGED = 0.005
# How far a channel must move for that pixel to count as moved, out of 255.
#
# **Three, not ten.** A twelve-step dissolve between two covers moves each
# channel by a few units per step, so a threshold set for "a title was
# replaced" is blind to the very thing being measured — the first cut of this
# script reported four redrawn frames inside a sixteen-frame crossing for
# exactly that reason. The film is lossless, so three is still well clear of
# noise.
STEP = 3
# The band that counts as "strictly between the two records".
MOVING = (0.02, 0.98)


def box(frame, rect):
    x, y, w, h = rect
    return frame[y:y + h, x:x + w]


def project(value, start, end):
    """Where `value` sits on the segment `start` -> `end`, as a fraction.

    A dot product rather than a per-channel ratio: a crossfade moves all three
    channels together, and projecting onto the line is the one reading that
    stays meaningful when a channel barely moves.  `None` when the two ends are
    the same colour — the honest answer to "how far along" when there is
    nowhere to be along.
    """
    span = end - start
    denom = float(span @ span)
    if denom < 1e-6:
        return None
    return float((value - start) @ span / denom)


def moved(a, b, rect):
    """The share of `rect`'s pixels that moved between two frames."""
    diff = np.abs(box(a, rect).astype(np.int16) - box(b, rect).astype(np.int16))
    return float((diff.max(axis=2) > STEP).mean())


def montage(images, path, pad=4, ground=(12, 13, 14)):
    """Lay `images` in a row on the room's own `wall`."""
    w = sum(i.width for i in images) + pad * (len(images) + 1)
    h = max(i.height for i in images) + pad * 2
    sheet = Image.new("RGB", (w, h), ground)
    x = pad
    for image in images:
        sheet.paste(image, (x, pad))
        x += image.width + pad
    sheet.save(path)


def main():
    frames_dir = pathlib.Path(sys.argv[1])
    out = pathlib.Path(sys.argv[2])
    build = sys.argv[3]
    paths = sorted(frames_dir.glob("*.png"))
    if len(paths) < 60:
        raise SystemExit(f"only {len(paths)} frames — the film did not record")

    sleeves, fields, edges = [], [], []
    title_step, sleeve_step, field_step, whole_step = [0.0], [0.0], [0.0], [0.0]
    previous = None
    for path in paths:
        frame = np.asarray(Image.open(path).convert("RGB"))
        sleeves.append(box(frame, SLEEVE).reshape(-1, 3).mean(axis=0))
        fields.append(box(frame, FIELD).reshape(-1, 3).mean(axis=0))
        edges.append(box(frame, EDGE).reshape(-1, 3).mean(axis=0))
        if previous is not None:
            title_step.append(moved(previous, frame, TITLE))
            sleeve_step.append(moved(previous, frame, SLEEVE))
            field_step.append(moved(previous, frame, FIELD))
            whole_step.append(moved(previous, frame, WHOLE))
        previous = frame

    # The two ends, read six frames in from each edge so a frame caught
    # mid-grab cannot become an endpoint.
    start_s, start_f = sleeves[6], fields[6]
    end_s, end_f = sleeves[-6], fields[-6]

    # **A film whose two ends are the same record proves nothing**, and is what
    # a race between the launch scan and `Play all` looks like from here: the
    # gestures all land, the app is healthy, and the record simply never
    # changed. Refuse it rather than report a crossing of zero frames as though
    # it were a finding.
    if float(np.abs(end_s - start_s).max()) < 8.0:
        raise SystemExit(
            f"the sleeve reads the same at both ends of the film "
            f"({[round(v) for v in start_s]} vs {[round(v) for v in end_s]}) — "
            f"the record never changed, so there is nothing to measure. "
            f"Re-shoot; the usual cause is the library scan not having finished "
            f"before `Play all`.")

    rows = []
    for index, path in enumerate(paths):
        rows.append({
            "frame": index,
            "ms": round(index * 1000 / 60),
            "name": path.name,
            "t_sleeve": project(sleeves[index], start_s, end_s),
            "t_field": project(fields[index], start_f, end_f),
            "title_moved": title_step[index] > CHANGED,
            "sleeve_moved": sleeve_step[index] > CHANGED,
            # Is the artwork filling the box the hero is drawn in, or is a
            # 320 px thumbnail standing in the middle of it?
            "art_fills_its_box": float(
                np.abs(edges[index] - fields[index]).max()) > 12.0,
        })

    def between(key):
        return [r for r in rows if r[key] is not None and MOVING[0] < r[key] < MOVING[1]]

    changes = [r["frame"] for r in rows if r["title_moved"]]
    # A title drawn across two grabs is one event, not two — and the frames of
    # a dissolve are not events at all: the wash crossing *behind* the type
    # moves the title box without a word of it changing. So a title move that
    # falls inside a sleeve crossing belongs to the crossing.
    redrawn_set = {r["frame"] for r in rows if r["sleeve_moved"]}

    def inside_a_crossing(frame):
        return any(abs(frame - f) <= 2 for f in redrawn_set) and frame > (changes[0] if changes else 0)

    events, previous_event = [], None
    for frame in changes:
        if previous_event is not None and frame - previous_event <= 4:
            continue
        if previous_event is not None and inside_a_crossing(frame):
            continue
        events.append(frame)
        previous_event = frame
    redrawn = [r["frame"] for r in rows if r["sleeve_moved"]]
    crossing, crossing_field = between("t_sleeve"), between("t_field")

    print(f"# {build}: {len(paths)} frames at 60 fps ({len(paths) / 60:.2f} s)")
    print(f"# sleeve ends: {[round(v) for v in start_s]} -> {[round(v) for v in end_s]}")
    print(f"# field  ends: {[round(v) for v in start_f]} -> {[round(v) for v in end_f]}")
    print(f"# track changes at frames {events} ({[round(f * 1000 / 60) for f in events]} ms)")
    print(f"# frames in which any sleeve pixel moved: {len(redrawn)} {redrawn}")
    # Per track change, how much of the sleeve moved in the second after it.
    # This is the negative case as one number: a record change repaints the
    # artwork, and a track change inside one record repaints nothing.
    labels = {}
    for event in events:
        labels[event] = ("the record changed"
                         if any(f for f in redrawn_set if 0 <= f - event <= 40)
                         else "the same record")
    for event in events:
        window = sleeve_step[event:event + 60]
        drawn = sum(1 for value in window if value > CHANGED)
        painted = sum(1 for value in whole_step[event:event + 60] if value > 0.0)
        print(f"# at the change on frame {event} ({labels[event]}): "
              f"the sleeve's pixels moved in "
              f"{drawn} of the next 60 frames, at most "
              f"{max(window) * 100:.1f} % of the probe at once; the *window* "
              f"was repainted {painted} times in that second")
    narrow = [r["frame"] for r in rows if not r["art_fills_its_box"]]
    if narrow:
        runs, start = [], narrow[0]
        for a, b in zip(narrow, narrow[1:] + [None]):
            if b is None or b - a > 1:
                runs.append((start, a))
                start = b
        print(f"# the artwork did NOT fill its box (a thumbnail standing in for "
              f"the hero) in {len(narrow)} frames: {runs}")
    else:
        print("# the artwork filled its box in every frame of the film")
    print(f"# frames strictly between the two sleeves: {len(crossing)}")
    print(f"# frames strictly between the two fields:  {len(crossing_field)}")
    span = None
    if crossing:
        span = (crossing[0]["frame"], crossing[-1]["frame"])
        print(f"# the sleeve's transition spans frames {span[0]}..{span[1]} = "
              f"{(span[1] - span[0] + 1) * 1000 / 60:.0f} ms of film")
    if crossing_field:
        edge = (crossing_field[0]["frame"], crossing_field[-1]["frame"])
        print(f"# the field's transition spans frames {edge[0]}..{edge[1]} = "
              f"{(edge[1] - edge[0] + 1) * 1000 / 60:.0f} ms of film")
    if crossing and crossing_field:
        drift = max(abs(r["t_sleeve"] - r["t_field"]) for r in crossing)
        print(f"# largest disagreement between the cover and the room: {drift:.3f}")
    # The wait between the engine naming the record and the picture starting to
    # move: `art::load_hero` decoding off-thread, which is what the transition
    # is deliberately started *after*.
    if span and events:
        before = [f for f in events if f <= span[0]]
        if before:
            print(f"# the hero's decode held the picture for "
                  f"{(span[0] - before[-1]) * 1000 / 60:.0f} ms after the track changed")
    print()
    print(f"{'frame':>5} {'ms':>6} {'t sleeve':>9} {'t field':>8}  note")
    for row in rows:
        ts = "-" if row["t_sleeve"] is None else f"{row['t_sleeve']:.3f}"
        tf = "-" if row["t_field"] is None else f"{row['t_field']:.3f}"
        note = []
        if row["frame"] in events:
            note.append("track changed")
        if row["sleeve_moved"]:
            note.append("sleeve redrawn")
        if row in crossing or row in crossing_field:
            note.append("<-- crossing")
        print(f"{row['frame']:>5} {row['ms']:>6} {ts:>9} {tf:>8}  {' '.join(note)}")

    # -------------------------------------------------------------- the figures
    # Five instants through the record change. On the `before` build there is
    # nothing between the ends to ladder, which is the point: both sets are cut
    # at the same offsets from the same event and only one of them has a middle.
    # **Every frame the app actually drew**, not a fixed cadence. A cut and a
    # dissolve differ in exactly how many distinct frames they spend between
    # their ends, so sampling at fixed offsets would draw a cut as a ladder by
    # showing the same repeated frame five times.
    pivot = span[0] if span else (events[0] if events else len(rows) // 2)
    drawn = [f for f in redrawn if span and span[0] <= f <= span[1] + 2]
    ladder = [max(pivot - 4, 0)] + drawn + [min(len(rows) - 1, (span[1] if span else pivot) + 6)]
    ladder = sorted(dict.fromkeys(ladder))

    def frame_at(index):
        return Image.open(paths[index]).convert("RGB")

    montage([frame_at(i).crop((ART[0], ART[1], ART[0] + ART[2], ART[1] + ART[3]))
             .resize((228, 228), Image.LANCZOS) for i in ladder],
            out / f"01-the-cover-crossing-{build}.png")
    montage([frame_at(i).crop((FIELD[0], FIELD[1], FIELD[0] + FIELD[2], FIELD[1] + FIELD[3]))
             .resize((72, 480), Image.NEAREST) for i in ladder],
            out / f"02-the-field-crossing-{build}.png")
    # **The middle of whatever ladder there is.** A cut's ladder is two frames
    # and has no middle to index — an earlier draft asked for `ladder[2]`, which
    # raised on the `before` build *after* printing its measurements, so the run
    # looked successful and the control build silently lost two of its four
    # figures. Evidence that goes missing only for the unflattering half is the
    # exact failure this directory exists to prevent.
    frame_at(ladder[len(ladder) // 2]).save(out / f"03-mid-crossing-{build}.png")

    # The negative case: a later track change, inside one record. Three frames
    # spanning it — the title moves and the picture does not.
    inner_events = [e for e in events if labels.get(e) == "the same record"]
    if inner_events:
        inner = inner_events[0]
        montage([frame_at(min(max(inner + d, 0), len(rows) - 1))
                 .crop((ART[0], ART[1], ART[0] + ART[2], ART[1] + 620))
                 .resize((228, 310), Image.LANCZOS) for d in (-4, 4, 13)],
                out / f"04-one-record-two-tracks-{build}.png")
    else:
        print("# NOTE: no within-record track change in the film — no negative case cut")


if __name__ == "__main__":
    main()
