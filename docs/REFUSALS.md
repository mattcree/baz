# baz — the owner's refusals

> **Only Matt's own words forbidding something belong in this file.** Nothing
> else. Not a design study's conclusion, not an agent's inference, not a
> critique's recommendation — those are *reasoning*, they live in `docs/adr/`
> and `docs/design/`, and reasoning does not forbid anyone anything.
>
> **The one rule above all of these: anything he asks for goes in the app.**
> If something here appears to stand in the way of something he has asked for,
> the request wins and the entry is wrong — struck through, dated, with what
> replaced it.
>
> This file used to be 367 lines and about thirty entries, almost none of them
> his. It had been read — by agents, and by me — as a body of law binding the
> person whose product it is, to the point where his requests were being
> weighed against it before being built. His verdict, 2026-08-10: *"I think the
> refusals doc is bizarre — I want to ensure that only my words explicitly
> forbidding something end up in there. anything I request should be in this
> app. that is the only rule."*

---

## Playback

**No resampling. 100% accurate reproduction.**

> *"yeah lets not resample anything. we want 100% accurate reproduction"*

**Amended by him, the same day**, and the amendment is as binding as the
refusal:

> *"I think it's okay if we can resample in cases where it simply won't play
> otherwise… but maybe worth showing a small info icon or something indicating
> that is happening (dont make it look like a warning as it will annoy people
> who are OCD about such things)"*

So: never silently, never by preference, only where the alternative is silence
— and **the indicator may not look like a warning.**

---

## The product

**Nothing that needs a server.**

> *"I wanna make a free alternative that costs nothing, that does pretty much
> exactly what these things do. Apart from anything that requires a centralized
> server."*

The competitors named were Roon, Plexamp, Audirvana and Swinsian. Anything of
theirs that works on the user's own files is fair game; anything that needs
someone else's computer is not.

---

## The interface

**No monospace, anywhere.** His choice, made explicitly when the type direction
was put to him.

**Nothing important buried in a modifier key.**

> *"burying things in modifier keys is not great..."*

Said when a `Ctrl`-click was proposed as the way to play a record from the
wall. Modifiers may *accelerate* something that has a visible control;
they may not be the only route to it.

---

## Development

**Tests do not make noise unless asked.**

> *"those tests that make noises are horrible. can you ensure you only run those
> when you absolutely need to"*

`BAZ_DEVICE_TESTS=1` or silence. A plain `cargo test --workspace --all-features`
makes no sound, and that is a rule rather than an accident.

---

## The two standing hard rules

Not prohibitions, but he has stated them twice as the things that actually
matter, so they belong beside the refusals:

> *"hard rules to me are mostly about responsiveness and a nice aesthetic etc"*
>
> *"ambient motion is fine as long as the performance remains top tier"*

**It must stay fast, and it must look excellent.** Where a rule anywhere in this
repository works against those two, the rule is wrong.

---

## What used to be here

About thirty entries of design reasoning — the accent discipline, the motion
budget, the surface rules, the artwork rules, the history posture, the
composition laws' rationale. **None of it is lost and none of it was his.** It
is argued where argument belongs:

| It was about | It is argued in |
|---|---|
| Places, surfaces, the bar's slots | ADR-0016, ADR-0022, ADR-0030 |
| The accent, depth, colour | ADR-0017, `docs/design/02-visual-language.md` |
| Motion, and what may move | ADR-0020, ADR-0029 |
| Artwork, sleeves, the hover veil | ADR-0024 §A1, `docs/design/13` |
| History, and what it may surface | ADR-0018 |
| Playback, the queue, silence at the end of a run | ADR-0023 |
| Playlists, and what one is | ADR-0024 |
| Controls, icons, the strip's budget | ADR-0026, `docs/design/10` |

Code comments across the repository cite `docs/REFUSALS.md` for entries that
now live in those documents. They are stale pointers, not lies — the reasoning
is real and findable. They get corrected as each file is next touched, rather
than in one sweep that would touch everything and prove nothing.

**Two entries were struck by him before this file was cut down**, and they are
worth keeping visible as the reason it needed cutting:

- *"baz has no side surfaces"* — written from his own *"I hate the sidebar"*,
  then contradicted by his *"let's do the ground work for adding a home page
  and left hand side bar"*. A refusal built from a preference he later changed
  his mind about, which then had to be argued with before he could have what he
  asked for.
- *"Sound from the wall is two presses, and that is a price, not a debt"* —
  written the same morning he decided it, struck the same afternoon when he
  designed the hover options. It never should have been a rule; it was a state
  of the product.
