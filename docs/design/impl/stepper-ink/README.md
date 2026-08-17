# The settings steppers ride the transport's ink ladder

**2026-08-17.** Backlog: *The settings steppers' marks do not ride the
transport's hover tween.*

Doc 10 §7 step 6 swapped the steppers' font `−`/`+` for the drawn glyph pair —
the same pair the transport uses — and left them at the resting ink. So the
button *ground* answered a hover and the mark inside it did not: two
identical-looking icon buttons behaved differently depending on which place you
found them in.

The entry deferred it, and was honest about the price: *"brightening their
marks on hover would need two more `motion::Control` identities and the
`mouse_area` wiring the transport carries"*. That is exactly what it needed,
and it turned out to be six rather than two, because there are three stepper
rows.

## What changed

`theme::glyph_ink` — the complete ladder, 0.57 rest / 1.00 hover / 0.75 press /
0.28 disabled, with the 90 ms tween — already existed and was already used by
the app bar, the bottom bar and the status glyph. The steppers were the one
icon button in the product still reading `theme::glyph_opacity`, which is that
same ladder with the pointer's part left out.

So: six `motion::Control` identities (`SettingsWorkersDown`/`Up`,
`SettingsPreampDown`/`Up`, `SettingsNoTagPreampDown`/`Up`), the two-line
`mouse_area` every other icon button carries, and `Ink` threaded into
`settings::view`.

**Six identities and not one**, which is the part worth stating: `Ink` answers
*which control the pointer is on*, so a single shared identity would have lit
all six marks whenever the pointer found any one of them — a fix that looks
right in a screenshot of one row and is wrong the moment there are two.

## The proof

`prove.sh` drives a release baz on a private Xvfb (all six XDG variables
redirected into a scratch tree), opens Settings → Vibe, parks the pointer well
away from the row, then parks it on the `+`.

![the pair at rest, then with the pointer on the plus](hover.png)

Top: at rest. Bottom: pointer on the `+`. Measured off the frames, peak value
of the mark's own pixels:

| | `−` | `+` |
|---|---|---|
| at rest | 137 | 137 |
| pointer on `+` | 137 | **232** |

The `+` brightens; **the `−` beside it does not move**, which is the identities
being distinct rather than merely present.

### Why the Vibe section and not ReplayGain

The first attempt at this proof used the Pre-amp row and measured **74 in both
frames** — no change at all. That is not the fix failing; it is the ladder's
first rule working. Xvfb has no sound card, so baz has no engine, so
`live` is false and both ReplayGain steppers are disabled — and *a dead control
is dead at every value of hover*, deliberately, because an affordance that
answers a pointer is claiming it can be pressed. 74 is the disabled reading.

`Workers` depends on the config alone, so it is live on any machine. Worth
recording because the misleading measurement came first, and a hover proof run
against a disabled control would have read as a failed fix.
