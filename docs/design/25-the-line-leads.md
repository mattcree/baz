# 25 — The line leads, and the words are a filter

The owner, 2026-08-16, having read note 23's evidence:

> *"if we treat words as just a kind of filter… the curves make more sense up
> front I think"*

That is the decision note 23 §8 refused to make on its own, and it is the
right one. This note records what moved and why, so the next person to open
this page does not have to reconstruct the argument from a diff.

## 1. What was wrong

Note 23 §5 stated it and did not act on it:

| | rests on | can it be wrong? | where the page put it |
|---|---|---|---|
| **the words** | a neural model's opinion about a phrase | **yes, and silently** | at the head, in the only body-size control, in **both** depths |
| **the line** | tempo, loudness, loudness variance, spectral centroid, rolloff, zero crossings, flatness | no — these are measurements | in the middle of the answer column, **advanced depth only** |

So the default page had **no curve on it at all**, and led with the control
that for two of six tested requests retrieved no better than chance
(`docs/design/impl/vibe-eligibility/`).

Nobody decided this. It followed from design 21 §2's *"there is one request,
and it is a sentence about sound"* — a good answer to a **different** problem
(mood and words reading as two separate inputs) that quietly became a claim
about importance.

## 2. What "a filter" means, precisely

It is what the engine has done since plan 22: **two stages**. The words draw a
pool; the line orders it. Moving the line reorders the same songs.

So this note changes **no engine behaviour whatsoever**. It changes the page
to describe the engine it already has. That is the whole reason it is safe to
do without waiting for the blind ballot: the disputed question — *how good is
the retrieval* — is untouched. What is settled is the question of **which
control the page should be built around**, and a control that can be checked
by ear beats one that cannot.

## 3. What moved

1. **The line is the page's question.** `How should it move?` is the one
   emphasis-weight heading, first in the reading order, and it stands at
   **both** depths. Simple mode has a curve now; it never did.
2. **The columns are what you set and what you will get** — the line and what
   narrows it on the left, the request in a sentence, the length, `Compose`
   and the list on the right. (This landed in two steps: the first arrangement
   put the line, the sentence and the list in one column, which read well and
   cost the list its permanent place. §3a item 3 is the correction.)
3. **The words are a filter column**, `NARROW IT DOWN`, at caption weight,
   with *"Optional. Leave it empty and Baz draws from everything it has
   heard."* under the field. The optionality is not a concession; it is the
   accurate description of a two-stage engine whose first stage may be empty.
4. **The stated request leads with the shape**: *Starting quiet and climbing
   the whole way, for about an hour, drawn from songs like "a slow warm
   pulse".* Same three clauses as before, in the order that says which of them
   the request is built on.
5. **The depths mean something different now.** Simple is *the line, a length,
   some words to narrow with*. Advanced adds the per-dimension lines, the
   vocabulary chips and the readouts. The line is no longer what you unlock;
   the query builder is.

## 3a. What the owner asked for once he could see it

Five corrections, the same day, and each of them follows from the line being
the page's subject rather than a setting on it:

1. **Ten points, not two.** *"Can we just make the line default to having
   let's say 10 points?"* Two points is a control you can tilt; ten is one you
   can draw with. It is deliberately past the point where every segment holds
   a song — an hour is around eighteen tracks — and that is his to spend: the
   cost of a line finer than the list is that the last of the detail cannot be
   expressed, not that anything breaks.

   Presets arrive at the same resolution, **without changing shape**. Every
   point a preset states is kept and the new handles are shared among the gaps
   in proportion to their width, so an inserted point always lands on a
   straight segment and reading the level anywhere gives the same answer. The
   first attempt sampled a plain even grid and `Waves` — which turns at 0.25
   and 0.5 — arrived visibly flattened.

2. **Advanced opens the five lines.** *"Can you make advanced mode open up the
   multiple curves."* It also closes the hole that came with putting the
   expander in the advanced depth: a page left expanded and switched to simple
   drew five lines with no control to collapse them.

3. **The list stands on the right at all times.** *"Show the playlist at the
   right at all times when the screen is wide enough."* So the columns are
   **what you set** and **what you will get**: the line and what narrows it on
   the left, and the request in one sentence, the length, `Compose` and the
   list on the right. The sentence sitting immediately above the button says
   exactly what pressing it will do, and both stay at the top of the page
   however tall the left column grows — five opened lines are 1 100 px, and
   the quorum's R11 asks for the commitment to be in *reach*, not merely
   present.

   Both columns now grow from their own floors rather than one being pinned,
   because the drawn line is the one thing on this page that is better for
   room.

4. **The blend got a name, and the shares got a referent.** *"The concept of
   the 'blend' of the curves isn't that clear."* It was not explained
   anywhere: the word stood beside each percentage as though the reader
   already had it, and in simple mode nothing said the single line was five
   lines at all — which the previous arrangement had hidden behind a tab and
   this one puts at the top of the page.

   `40% OF THE BLEND` reads `40% OF THE DECISION` now, because a share has to
   be a share of something a reader can name. And the control says what it is,
   once, at the head of it:

   > *One line asking for five things at once: energy counts most, then tempo,
   > then brightness and dynamics, and texture least. Advanced shapes each of
   > them on its own.*

   > *One line each — and a song cannot be in five places at once, so where
   > the lines disagree the shares below settle it. That is why the dots track
   > a 40% line closely and a 10% line loosely.*

   **The second sentence is also the answer to a bug report that was not
   one.** The owner, on the opened lines: *"the stuff is not conforming to
   each."* The per-lane dots are each track's real reading on that dimension —
   checked in `baz_vibe`, not assumed — and they do not sit on their line
   because the walk satisfies the *weighted* request rather than each line
   separately, which it cannot do when the lines disagree. Nothing was broken.
   What was missing was the sentence saying so, and without it a working
   control looked like a failing one.

5. **One graph, with tabs, and the points stopped being turns.** *"I like the
   idea of all lines being on the same graph and a way to kinda toggle between
   all and individual… then selecting each individually to be able to
   configure that line."*

   Five stacked canvases — 1 100 px of them — became one, with a row of tabs
   over it: `All five`, then each line beside its share. Picking one makes it
   the line you drag, drops what it measures and any flat-axis warning
   underneath it, and leaves the other four **on the same canvas as ghosts**.
   That last part is the point: where the ghosts part from the line in front,
   that gap is the disagreement the shares are settling, and it is the only
   place in the feature where that is visible rather than described.

   A tab is a **view**, not a mode: pressing one changes what you can drag and
   nothing about the request, which is what makes them free to press. Coming
   back to one shape is its own act now, named for what it does, rather than
   the side effect of closing an expander.

   On `All five` a drag moves every line **by what the pointer moved** rather
   than setting them all to where it landed. The difference only shows once
   the lines have been pulled apart — which is exactly when the old behaviour
   would have silently thrown that work away.

   And the point count reads `2 3 4 … 10` under `POINTS`, not `Straight`,
   `1 turn`, `2 turns`. *"The term 'turns' is not correct in this case I
   think? more like just points on a curve."* Ten points on a straight line
   has no turns in it at all, so the label was a claim about shape made by a
   control that only sets a number of handles.

And one thing that fell out rather than being asked for: **the match count
moved to both depths.** It was advanced-only when the words were the request
and the readouts were the query builder's own. *What did my filter catch* is
the plainest question a filter can be asked — and since advanced now opens
five curves above it, advanced was the depth where the answer was hardest to
reach, which is the opposite of what hiding it there assumed.

## 4. What did not move, deliberately

- **Prose was not deleted, and the moods were not narrowed to instruments.**
  Note 23 §6's options B and C go further than the evidence: §7a showed a tag
  filter cannot reproduce what the model does, and half the committed requests
  have no genre word to filter on at all. Demoting prose is supported; deleting
  it is not.
- **Nothing about eligibility, ranking or the walk.** See §2.
- **The door.** Its six moods still set words, shape and length together,
  which is what a preset is. A mood is not a filter, and the door is not this
  page.

## 4a. Simple and Advanced is now a mode over one row of chips

Worth stating plainly, because it is the consequence of §3a item 5 and it is
his call rather than this note's. The advanced depth used to gate the
per-dimension lines, the vocabulary and the readouts. The lines moved onto the
graph's own tabs, where they are a view rather than a depth. The match count
went to both depths, because *what did my filter catch* is not an advanced
question — and the depth that opens five curves was the one where it had been
hardest to reach.

What is left behind the tab is **one row of six words**. A mode is a promise
that there are two pages to learn; this one now promises that for a chip row,
which is a disclosure at most.

## 5. What still needs the owner

The blind ballot (note 23 §7 experiment 1, plan 22 §0.1). The harness, the
consented 72-track corpus, the diversity-matched random control and 36
anonymous candidate lists are all in the tree with the ratings unfilled. It
answers the one question no measurement here can: whether the model's idea of
*warm hypnotic music for driving at night* is anybody's.

This note makes that question **less expensive to get wrong**, because the
page no longer rests on the answer. If the retrieval turns out to be poor, the
feature degrades to *draw a shape over your whole library*, which is a real
thing that works. Before this note it would have degraded to a text box that
lies.
