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
   every listener, at every depth there then was. The default page had no
   curve on it at all before this.
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
5. **The depths meant something different, and then stopped meaning enough to
   keep.** See §4a: the mode is gone and the words fold away instead.

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
   > then brightness and dynamics, and texture least. Pick one to shape it on
   > its own.*

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

   **`All five together` took three tellings, and the first two answers were
   both wrong for the same reason.** The owner: *"if you go back to all five
   mode it should start to control all lines"*, then *"basically the all thing
   should be controlling all lines"*, then — plainly — *"if I've edited any of
   the individual lines and then go back to all five, it does not snap the
   previously edited lines to the 'all five' line."*

   The first answer made the drag move every line by the pointer's delta,
   preserving whatever spread had been built. The second drew all five at full
   strength on that tab so it looked like all five were held. Both were
   defensible; both were built on the same wrong premise, which was that a tab
   is a **view** — press it, change what you can drag, change nothing about
   the request.

   **It is not a view. `All five together` is a shape**, and choosing it is
   choosing to have one, so every line returns to it. That is lossy, and it
   should be: the alternative is what he kept running into, where a control
   labelled *all five* quietly presided over five different curves. The chip
   says what it will do before it is pressed, and there is no second control
   for gathering the lines because the tab is it.

   The lesson is worth keeping past this page. Twice I answered *what should
   this control do* by making the existing behaviour more visible, when the
   complaint was that the behaviour was wrong. A user saying the same thing
   three times is not failing to see the picture.

   And the row says which of the two you are in. Six chips in a line read as
   six peers, one of them lit; the *all* chip stands on its own row with the
   word `or` after it, above the five. *"It should be a bit more obvious that
   it's an alternative i.e. All five OR the rest."*

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

## 4a. Simple and Advanced is gone

It was a mode over one row of chips by the end, which is not a mode. The
advanced depth used to gate the per-dimension lines, the vocabulary and the
readouts; the lines moved onto the graph's own tabs where they are a view, and
the match count went to both depths because *what did my filter catch* is not
an advanced question. That left six words behind a tab, and a mode is a
promise that there are two pages to learn.

**What replaced it is a disclosure, which is the honest shape of the same
idea.** `Only certain songs` folds the whole words band away — the field, the
count, the moods and the vocabulary — and unfolds it when pressed or when a
request arrives with words already in it. Folded, it is one chip and one line
of explanation; unfolded it is everything the advanced depth used to hold.

The difference is not cosmetic. A mode says *there is another page*. A
disclosure says *there is more of this one*, and it is reversible in the place
you are standing.

## 4b. The economy pass, and where the press went

The owner, testing it: *"the information architecture and where buttons are
etc. is not good e.g. the 'compose' button is on the right? it should be on
the left… please be more economical with space etc. and make sure we tune the
copy for the lowest common denominator."*

He is right about the button, and it was §3a item 3's fault: the sentence and
the press went to the answer column to stay above five stacked curves. The
curves are one canvas now, so the reason has expired. **One column is the
request and one is the answer, and the act that turns the first into the
second belongs at the foot of the first.**

What the space came from, in order of how much:

| removed | rows |
|---|---|
| the Simple / Advanced tabs and their explanation | 2 |
| the model sentence — *the line picks… the length picks…* | 1 |
| the `BAZ WILL LOOK FOR` block, folded to one quiet line under `Compose` | 3 |
| the words band, folded away until asked for | 5 |
| the third row of axis labels | 1 |

`Compose` sits a little over halfway down a 980 px window with the words
folded, where it used to be off the bottom of it.

**And the copy came down a reading age.** *"Spectral centroid, rolloff and
zero crossings"* is now *"How bright or dark a song sounds."* *"Baz drew 24 of
24 to choose from"* is *"Using 24 of your 24 songs."* *"Nearest your words: …
— if these are not what you meant, it has not understood the phrase"* is
*"Closest: … If these look wrong, try different words."* Nothing became less
true; several things stopped needing a second reading.

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
