# 26 — The site

*The brief, written 2026-08-20. Nothing is built yet, by instruction: "lets not
necessarily do it until our backlog is settled, but lets plan it in".*

The owner's words are the whole specification and they are unusually precise:

> lets make a really nice enticing website showing off the features of the app…
> and ensure we really make it nice. like, super minimal, no trappings of AI
> generated sites, and just make the app front and center

Three constraints, and they are not decoration on a fourth. **Minimal**, **not
AI-looking**, and **the app front and centre**. Everything below is those three
followed to where they lead.

## 1. The app is the page

The single decision this document exists to make. A page that *describes* baz
is a page competing with every other player's page, and the thing baz has that
they do not is that it looks like something. So the hero is not a headline over
a screenshot. **The hero is a screenshot, at full bleed, with the words placed
in it rather than above it.**

That has a consequence this repo has to pay for before the page is written:
the screenshots have to be *good*, and they have to be **reproducible**. A
marketing screenshot taken by hand once is stale by the next release and
nobody notices until somebody downloads the app and it does not match.

**Wanted, and buildable now:** a `scripts/` capture that runs the real binary
headless (Xvfb, an isolated XDG, a seeded library of freely-licensed covers)
and writes the page's images at a fixed window size, one per shot, on demand
and in CI. Every one of this session's proof shots was taken that way already;
what is missing is a *committed corpus* to take them of, because the owner's
own library cannot ship.

## 2. What "no trappings of AI generated sites" rules out

Stated as a list because it is easier to check than to describe, and because
the failure mode is that each of these individually feels like a reasonable
choice:

- a gradient hero, and in particular purple-to-blue;
- three feature cards in a row, each with an icon above a two-line blurb;
- an emoji as a section marker;
- Inter, Space Grotesk, or any of the current defaults, as the display face;
- everything centred;
- a "Trusted by" strip, a fake testimonial, or a badge nobody issued;
- rounded cards with an accent bar or rail;
- copy in the register of *Effortlessly organise your music library with
  powerful, intuitive tools.*

The positive form of the same rule: **the site should look like the
application it is for.** baz has a typographic identity already — a serif
italic for a work's title, tracked caps for a heading, four surface planes,
hairlines, square artwork, barely-rounded controls, and sixteen rooms one of
which the page should simply *be*. The site's palette is a `Palette`, exported
from the same source of truth rather than eyeballed from a screenshot, and its
type ramp is `theme`'s.

## 3. What the page says, in order

Minimal is a claim about the number of things, not about their size. Five
blocks:

1. **The record, playing.** Now playing at full bleed, the jewel case turning,
   the title in the serif. Over it: the name, one sentence, and the download.
   The sentence is not a tagline — it is what baz is, which the README already
   states better than a slogan would.
2. **The wall.** One screenshot of the collection, because the second thing
   anyone wants to know about a music player is *what does my library look
   like in it*, and this is the answer nothing else in the field gives.
3. **The three things nobody else does**, each a screenshot with one paragraph
   and no icon: the vibe line (draw a shape, get a playlist), *What baz heard*
   (the analysis reading its own library back), and the signal path readout —
   bit-perfect, stated, with the equaliser and ReplayGain that sit on it.
4. **The rooms.** Sixteen, as a strip of small captures — this is the one place
   a grid is honest, because the content genuinely is *sixteen of the same
   thing*, and it does the work a paragraph about theming cannot.
5. **Get it.** The three routes `docs/INSTALL.md` already documents, the
   platforms, the licence, the repository. No newsletter, no waitlist.

No navigation bar: five blocks do not need one, and a nav is the first thing
that makes a one-page site look like a template.

## 4. How it is built and shipped

- **One HTML file and one stylesheet**, hand-written. No framework, no build
  step, no analytics, no fonts fetched from a third party — the faces baz
  already ships are the faces the page uses, self-hosted, which is also the
  only way the type can match the screenshots.
- **Published by the release workflow that already builds the archives**, to
  GitHub Pages, so the download links and the version on the page cannot
  disagree with the release they point at. That is the one piece of automation
  worth having and it is small.
- **Accessible and light by construction**: it is text and images. A page for
  an application that cares this much about a focus ring should not itself be
  unusable without a mouse.

## 5. What has to exist first

In order, and each is useful on its own account:

1. **A committed screenshot corpus** — freely-licensed covers and a seeded
   library, so a capture is reproducible by anyone and legal to publish.
2. **The capture script**, promoted from this session's scratch harness into
   `scripts/` with the shots it takes named and pinned.
3. **A palette export** — the room the page uses, emitted from `theme` rather
   than transcribed, so the site cannot drift from the product.

None of the three is website work. All three are things this repo should have
anyway, which is the argument for writing this brief now and building the page
later.
