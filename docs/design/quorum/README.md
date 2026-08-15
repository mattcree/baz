# Quorums

A quorum is a room of invented experts, each with one hat, arguing about a
design that is already on the table. It exists to find what the proposal
missed — not to produce the proposal, which is somebody's job with their name
on it.

The owner asked for the first one on 2026-08-15, after reading a redesign of
the Vibe page: *"it still feels like it hasn't addressed everything. can you
create a quorum of domain experts and UX experts and have them discuss this in
a JSONL file as a chatroom."*

## What is here

| File | Room |
|---|---|
| `2026-08-15-vibe.jsonl` | The Vibe playlist page, next phase — 89 messages, 12 resolutions, 4 questions left for the owner. Reviews `docs/design/19-vibe-next-phase.md`. |

## The format

One JSON object per line, three kinds, in this order:

**`room`** — the first line. The subject, why it was convened, what everybody
had read, and the participants: a `handle`, a real-sounding `name`, the `hat`
they wear, and one line on what that hat is for. The hats are the point. A room
of nine generalists produces one opinion nine times.

**`message`** — `seq`, `at` (minutes into the session, not a wall clock),
`from` (a handle), and `text`. Read in `seq` order; nothing is threaded,
because a room isn't.

**`resolution`** — what the room changed about the proposal, with `backed_by`
naming who argued for it. `changes_proposal: true` means the design note has to
be amended, and it was.

**`open_question`** — what the room deliberately refused to decide, with the
`provenance` of who raised it and where it got to. `owner: "matt"` because
these are the ones that are not a designer's to settle: consent, taste, and
scope.

Read the whole thing:

```sh
jq -r 'select(.type=="message") | "\(.at)  \(.from): \(.text)"' \
  docs/design/quorum/2026-08-15-vibe.jsonl
```

Just the outcome:

```sh
jq -r 'select(.type=="resolution") | "\(.id)  \(.title)"' \
  docs/design/quorum/2026-08-15-vibe.jsonl
jq -r 'select(.type=="open_question") | "\(.id)  \(.title)"' \
  docs/design/quorum/2026-08-15-vibe.jsonl
```

## What a quorum is not

It is **not evidence**. Nobody in the room measured anything; the numbers they
quote come from measurements already in this repository
(`docs/design/impl/vibe-memory/`, `docs/design/impl/contour/`) and where a
number does not exist, the transcript says so rather than inventing one —
Lena's refusal to state a per-track analysis rate is the example to copy.

It is **not a decision**. The resolutions are what the room would do; the
questions are what it would not. Both go to the owner.

It is **not a substitute for a user**. Toby is a proxy, and a proxy is a
stand-in for the person you have not asked yet.
