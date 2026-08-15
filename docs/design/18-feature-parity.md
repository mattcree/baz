# 18 — Feature parity: what other players have, and what baz is missing

The owner, 2026-08-15: *"I think we should also do a feature parity run to see
what other players out there have and what we're missing. we should size up
the most important features that you think most people can't do without,
analyse them, get them in the backlog and just do them."*

This is that run. The comparison set is the players baz is actually measured
against by the people who would use it: **foobar2000** (the stated
inspiration), **MusicBee**, **Strawberry/Clementine**, **Quod Libet**,
**Rhythmbox**, and the mainstream streaming clients for the habits listeners
bring with them.

## 0. How this list was made

Two questions per feature, in this order:

1. **Would a listener notice its absence in the first hour?** That is the bar
   for *can't do without* — not whether a competitor has it.
2. **Does it fit baz's own promises?** Offline, no account, no telemetry, no
   network requests baz did not have to make. A feature that fails this is not
   a gap; it is a decision, and it belongs in the refusals below rather than in
   the backlog.

Anything that survives both is sized and queued. Everything else is recorded
with the reason, because *"we chose not to"* is worth as much in six months as
*"we haven't yet"*.

## 1. What baz already has

Stated first, because the list below reads like a deficit otherwise, and it is
not: gapless playback; ReplayGain with a visible signal path; bit-perfect
exclusive output; a library scan that survives moved and renamed files;
instant search over tracks and albums; album, artist and playlist places;
ordinary `.m3u8` playlists with undo and a trash-first delete; a queue you can
edit; favourites; shuffle **and repeat** (see §2.1); MPRIS and desktop media
keys; four themes plus a JSON schema for your own; a Now Playing surface with
real signal readings; local semantic playlist generation with a drawn contour;
a play-history ledger; per-place session restore; and a debug readout of its
own memory and CPU.

## 2. The gaps, ranked by *would a listener notice in the first hour*

### 2.1 Repeat the list — **shipped 2026-08-15**

baz had `repeat_one` and nothing else: a track could repeat, a run could not.
That is the state most listeners mean by the word *repeat*, and its absence
would be noticed in the first hour by anyone who leaves music on. Now one
control cycling **off → the list → this track**, with the two lit states
carrying different marks (the same loop, with and without a `1`).

Engine: `Repeat::{Off, All, One}`, and repeat-all restarts the *traversal's*
top, so a shuffled run repeats the order it drew rather than jumping to
whichever file is first in the list.

### 2.2 Multi-select and bulk actions — **item 62**

Every player in the set lets you rubber-band or shift-click a run of tracks
and then do one thing to all of them: queue, add to a playlist, remove. baz's
selection is deliberately *one* content item (ADR-0017's select-then-activate
grammar), which is right for activation and wrong for the moment a listener
wants twelve tracks in a list.

Noticed within the hour by anyone building a playlist by hand. Sized medium:
the selection type, the shift/ctrl grammar, and the acts that then apply to a
set rather than to an item.

### 2.3 A sleep timer — **item 63**

Small, universal, and absent. Every phone player and most desktop ones have
it; it is the one feature a listener asks for at midnight and cannot improvise.
Sized small: a bounded countdown that pauses at zero, with a visible remaining
time and a cancel.

### 2.4 Lyrics — **item 64**

Embedded (`USLT`) and sidecar (`.lrc`) lyrics are the common case and both are
**offline**, which is what makes this the one "metadata" feature that fits
baz's promises without a network request. Timed `.lrc` can follow the
playhead; plain text is a panel.

Sized medium: a reader, a place on Now Playing, and the honest empty state.

### 2.5 Ratings beyond a heart — **item 65**

baz has favourites (binary). foobar2000, MusicBee and Quod Libet all carry
0–5 stars, and listeners with large libraries use them to build lists. The
question is whether it earns a second axis beside Favourites or replaces it;
that is the analysis in the backlog entry.

### 2.6 Rule-based playlists ("smart playlists") — **item 66**

*Everything added this year, over 4 stars, not played in six months* is a
staple of foobar2000 and MusicBee, and baz has the data for all of it (the
index, the ledger, favourites). It is the biggest *capability* gap in the list
and the one that most rewards baz's existing honesty about what it knows.

Sized large: a rule model, a place, and the question of whether such a list is
a saved query or a materialised `.m3u8`.

### 2.7 Tag editing — **item 67**

foobar2000's mass tagger is one of the reasons people stay with it, and
baz's own VISION names *"mass-capable tagging"* as inherited identity. It is
the largest and the riskiest item here: baz would be writing to the listener's
files for the first time.

### 2.8 A folder view — **item 68**

Players in this set all offer *browse by folder* beside *browse by tags*, and
for listeners whose libraries are organised by hand it is how they navigate.
baz's wall is entirely tag-derived today.

### 2.9 Crossfade — **item 69**

Common in the streaming clients and in MusicBee. baz is album-first and
gapless, where crossfade is actively wrong; but for shuffled listening it is
what people expect. Sized medium, and its interaction with gapless has to be
stated rather than discovered.

### 2.10 Drag and drop from the file manager — **item 70**

Dropping a folder onto the window is how a lot of people first try a player.
baz has internal drag (`crate::drag`) but accepts nothing from outside.

## 3. Deliberate refusals, and why

These are *not* backlog items. They are recorded so nobody re-opens them
without new information.

- **Scrobbling (Last.fm/ListenBrainz)** — an account and a network request per
  play, which is the opposite of baz's stated promise. The play ledger is
  local and already exists; if this is ever revisited, it is as an explicit,
  per-session export rather than as a background sender.
- **Online metadata and cover fetching** — the same argument. baz reads what
  is in your files and says so when there is nothing.
- **CD ripping, transcoding, format conversion** — a different application,
  and one with a mature free field (EAC, whipper, ffmpeg).
- **Podcasts, and streaming services** — a subscription client is not this
  product; internet *radio* is arguably different and is the one to revisit
  first if any of this is.
- **A plugin API** — VISION's "design early, stabilise late", with foobar's
  ecosystem fragility as the cautionary tale. Not a parity gap; a staging
  decision.

## 4. What this run changes about the order of work

The two items that should be done before anything else in §2 are **multi-select
(2.2)** and **the sleep timer (2.3)**: one is a workflow floor that every other
list feature stands on, and the other is an evening's work for a feature people
genuinely miss. Rule-based playlists (2.6) is the biggest prize and the one
worth doing properly rather than quickly.
