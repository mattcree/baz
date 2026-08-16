# 24 — What Baz heard

> **Built, 2026-08-16.** §2's named extremes, §3's never-played count, §4's
> degenerate-axis flag and §7's items 2 and 3 are all shipping; the receipts
> are in `docs/design/impl/what-baz-heard/`. §7 item 1 — a vocabulary
> computed from the listener's own library — was **attempted three times and
> abandoned**, and the negative result is recorded in
> `crates/baz-vibe/src/bin/word-probe.rs` rather than hidden: CLAP's
> text-audio similarities are not comparable across prompts, so no ranking of
> candidate words against one library survives contact with the library's
> actual contents. The note below stands as written; this is what became of
> it.

An hour of listening currently buys the *ability* to compose and shows nothing
for itself. This note is about whether it should show something, what would
keep that from becoming a nightmare — and, by §7, why a local player can be
far more personal than a hosted one without becoming one.

## 1. The rule, located rather than repealed

baz already refused this genre once. The Now Playing facts strip carries a
standing rule: *it never emits streaks, rankings, congratulations or listening
totals.* On the face of it, that forbids this whole note.

The owner, 2026-08-16: *"as long as we don't turn into google or whatever
nightmare corp we can name, the rules are made to be broken."*

He is right, and the useful thing is to say **what** was actually wrong, so the
rule can be aimed rather than dropped. What makes a Wrapped bad is not that it
counts:

- it is computed about **you**, on somebody else's machine, and fed back as a
  product;
- it is engineered to be **shared**, because that is how it markets the
  platform;
- it is **flattery** designed to make you feel seen so that you stay
  subscribed;
- it **gamifies** listening, which turns a pleasure into a score.

None of that machinery exists here. There is no account, no network, no
retention to optimise, nothing to farm and nobody to share with. The rule was
written to stop baz *becoming* that, and it was written for a strip that
cycles facts **while music plays**, where a streak would be gamification at the
worst possible moment.

**So the line is: rank the music, not the listener.** *Your loudest record* is
a fact about a record. *You listened for 40 000 minutes* is a fact about you,
and that is the one that turns into a scoreboard. The old rule keeps its full
force where it was made.

## 2. The job

Not *here's your music*. **Here's what I heard — check me.**

That framing is what stops it being decoration, and it decides the content.
The most valuable things on this page are not summaries but **falsifiable
claims about specific records the listener already knows**:

```
Quietest        Arvo Pärt — Spiegel im Spiegel
Loudest         Slayer — Raining Blood
Fastest         …
Slowest         …
Tempo runs 91 to 167 BPM, centred on 127.
```

If baz names an ambient piece as the loudest thing in the library, the listener
knows in one second that the analysis is broken. If it names the metal record,
it has earned something. **An aggregate cannot be graded; a named record can**
— which is the same pattern as every other part of this feature that works: the
row words (`loud · fast · swinging`), the three closest matches under the
field. Name something specific and let the listener mark it.

This is also the only place the analysis can be audited **without composing**,
which matters for a step that takes an hour before anything else is possible.

## 3. The doorway

The strongest item is not a measurement at all. baz keeps a play ledger, so it
can say:

> **You have never played 3 412 of these.**

That is not flattery, it is not about the listener's habits, and it is
*actionable* — it is the one line on the page that leads somewhere. It is also
already a wanted feature: `WORK.md` item 76, *lean towards what you haven't
played*, which the quorum proposed and which is still waiting on the owner's
word.

Pairing them answers the question this note opens with — **is it worth wanting
twice?** A profile of measurements is a one-time curio. A never-played count
changes as you listen and is a route back into your own collection, so the
page has a reason to be revisited.

> **Built and removed, 2026-08-16.** This section is wrong, and the flaw is in
> its first sentence: *baz keeps a play ledger.* It does — an eight-day one.
> Measured on the owner's library the day this shipped: 864 plays over 262
> distinct songs, against 5 076 analysed tracks. So the line would have read
> *you have never played 4 814 of these*, which is a fact about **how long baz
> has been installed** wearing the clothes of a fact about its owner — and it
> sat among named records that can be graded in a second, borrowing their
> credibility. The owner, shown the numbers: *"it's irrelevant."*
>
> The lesson generalises past this line. §2 argues the valuable items here are
> **falsifiable claims**, and a claim is only falsifiable if the reader can
> tell what it is a claim *about*. This one could not, and no wording fixed
> that — *Baz has watched you play 262 of these* is accurate and still useless.
> `docs/WORK.md` item 76 carries the measurement.

## 4. What it must not claim

**The units problem.** *"Most varied in texture, least in brightness"* is the
line that would naturally go here, and it cannot be defended: those are raw
bliss features in different units — spectral flatness against loudness
variance — so the comparison is arithmetic without meaning. The honest form is
narrower and one-sided: flag a dimension only where the library is genuinely
**degenerate** on it — *your music barely varies in tempo, so that line will
not do much* — which needs no cross-dimension comparison at all.

That flag is worth having on its own account, because a rank axis spreads
whatever it is given across the full scale by construction. Draw a tempo curve
over a library with one tempo and the dots track the line perfectly while
nothing about the music changes, and nothing on screen says so today.

**The extremes may be junk.** The loudest track by mean loudness could be a
mastering artefact or a damaged file. That is arguably a feature — it surfaces
bad data where nothing else would — but it should not be presented as a
verdict on a record.

**No instrument claims yet.** *"Mostly guitars and drums"* is computable from
the CLAP embedding and measured well for instruments (3.5–4.1× genre
concentration, `docs/design/impl/vibe-eligibility/`). It is still inference,
and it should wait for `docs/design/23-the-three-dimensions.md` to resolve
whether the semantic half earns its place at all.

## 5. Where it lives

On the door, where the cost was paid. It already says *"Baz has heard all
5 076 of your songs"*; this is that line earning its space. **A few lines, not
a dashboard** — the page it sits on has been called overwhelming once already,
and a chart here would be the first thing to cut.

## 6. Why this one is worth doing before most of the backlog

It is **robust to the doc 23 decision.** Every fact above except the
instruments comes from conventional measurement, not from the semantic model.
If the words turn out not to earn their place and semantic retrieval is
dropped, this does not merely survive — it becomes the main evidence that the
listening step did anything at all.

## 7. The bigger idea this is one instance of

The owner, while this note was being written: *"it might be fun for their
experience in this app to be wholly driven by their own library and
interests."*

That is a larger claim than a profile page, and it is worth stating as a
principle, because it is the natural identity of a local player and the exact
opposite of the streaming model — where the application is identical for
everyone and **you** are the variable being optimised. Here the application
could be the variable and the library the constant.

**And locality is what makes it safe, which is the whole reason it is
available.** The owner: *"if we keep it local, we can do anything."* He is
right, and it is worth being precise about why, because the same behaviours
are creepy or benign depending on one fact:

| what baz would do | in a cloud product | here |
|---|---|---|
| know every file, and be opinionated about it | a profile it sells | it read your folder, which it had to |
| know what you have never played | a retention lever | a shelf you forgot about |
| derive its own vocabulary from your collection | a model trained on you | arithmetic on your disk |
| notice what you reach for | targeting | knowing your taste |

Nothing here leaves the machine, so none of it can be sold, shared, subpoenaed
or A/B tested against you. **The intimacy is affordable precisely because it is
inert.**

Which cuts the other way too, and is the obligation the boldness carries: the
moment any of it phones home, every item in the right-hand column becomes the
left-hand one. This principle is not *baz may be nosy*; it is *baz may be nosy
because it stays put*, and the second clause is doing all the work.

**No generic content anywhere.** Every example, every suggestion, every
default drawn from what the listener actually owns. Nothing a stock photo.

The instances, in order of how ready they are:

1. **The vocabulary should be computed, not constant.** This is the sharpest
   one, and it is embarrassing that it is not already true.
   `docs/design/impl/vibe-eligibility/` chose the chips by measuring 27
   candidates against *the owner's* library — and then the winners were
   hardcoded as six constants for everybody. A listener who owns no guitars
   is offered `electric guitars`; a listener with two thousand electronic
   records is not offered `drum machine` because it lost a contest held on
   somebody else's music.

   The fix is the measurement that already exists, run locally: embed a
   larger candidate vocabulary once (~60 words, a second of work, cached),
   score each against the library's own embeddings, and keep the ones that
   genuinely separate *this* collection. Everything needed is in the analysis
   store. The result is a vocabulary that is about the listener's music
   rather than about a test corpus.

2. **A mood should not be offered if the library cannot answer it.** `Party`
   on a collection of solo piano is a button that produces a disappointment.
   The eligible count already answers this — it is one embed per mood, once,
   after listening — so a mood could be quiet, or absent, or say what it can
   actually draw from.

   > **Built, measured, and removed the same day.** *"The eligible count
   > already answers this"* is the sentence that was wrong, and it was wrong
   > for the same reason item 1 above was. `vibe-spread` against the owner's
   > 5 076-track library, which contains no gregorian chant, no bagpipes, no
   > gamelan and no Mongolian throat singing:
   >
   > ```text
   > request                                      pool  top cos
   > warm hypnotic music for driving at night      211    0.555
   > calm instrumental music without vocals        157    0.620
   > upbeat energetic danceable music              196    0.620
   > gregorian chant                               175    0.697
   > bagpipe marching band                         246    0.489
   > traditional javanese gamelan                  187    0.589
   > throat singing from mongolia                  221    0.603
   > ```
   >
   > The four things the library does not contain draw pools of 175–246,
   > squarely inside the range the six real moods draw (157–252), and
   > `gregorian chant` returns the **highest** top similarity of the lot.
   > Neither the pool size nor the similarity level distinguishes a request a
   > library can answer from one it cannot, and the ranking is floored at
   > `KNEE_FLOOR` anyway, so *"only 24 songs to draw from"* was very nearly
   > unreachable as well as meaningless.
   >
   > There is no honest signal here on the evidence available. **CLAP
   > text-audio similarities are not comparable across prompts** — the same
   > wall item 1 hit three times — and *does this library contain X* is
   > exactly a cross-prompt question. It could be answered by tags for the
   > moods that have a genre word, but half of them do not
   > (`docs/design/23-the-three-dimensions.md` §7a), so that would be a
   > control that worked on three tiles and not the other three.

3. **The field's placeholder should name their own music**, not
   `warm hypnotic music for driving at night`. An example built from a record
   the listener owns teaches the same thing and proves the library is
   understood.

4. **Degenerate axes**, from §4 — already argued for, and the same idea:
   the control admits what this particular collection can and cannot do.

The common thread is that every one of these is baz **admitting what it knows
about the collection in front of it** rather than presenting a fixed surface
and hoping. It is the same instinct as this note's own framing — *check me* —
applied to the controls instead of the readouts.

**The cost to watch.** A surface that differs per library is a surface nobody
can screenshot, document, or support in general terms — every bug report
becomes *"which chips did you have?"*. That is a real price and it argues for
a **fixed frame with derived contents**: the rows and their labels stay put,
what sits in them is the listener's.

## 8. The honest doubt

This may still be dressing up a toll. The test is whether somebody who never
composes would want it, and the answer is *once* — twice, if the never-played
count is there. That is a real but modest claim, and it should not be oversold
into a reason for the hour of analysis. The reason for the hour is the
feature; this is what makes the hour legible.
