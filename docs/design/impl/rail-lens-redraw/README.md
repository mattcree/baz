# The index rail's lens drew once and then froze

The owner, twice. First: *"the right hand rail is acting strangely."* Then,
with the detail that made it findable: *"when mousing over the rail on the
playlist and library view the zoom doesn't really seem to work. it sometimes
zooms and the other times it doesn't."*

## What was actually happening

`spine.rs`'s module docs said it plainly, and the sentence was the bug:

> There is no tween, no clock, no subscription and no message: **iced requests
> a redraw for every window event**, so the lens moves exactly while the
> pointer does and costs nothing while it rests.

That was true of iced 0.13. baz migrated to **0.14**, where `Shell`'s redraw
request defaults to `RedrawRequest::Wait` and a widget that wants a frame has
to ask. The spine never asked, because its whole design is *no state, no
message* — and a widget that publishes neither gives the runtime no reason to
draw.

`groove` and `needle` are the project's other two hand-built pointer widgets
and were unaffected for exactly that reason: they publish a message on cursor
motion, so their frames arrive as a consequence of the shell's own update. The
spine is the only widget in baz whose **own** appearance is a function of the
live cursor with nothing published. (`menu::Area`, `drag::Source` and
`window_frame`'s wrappers take a cursor but only forward it to their children.)

## Measured, before

`rail.sh`: 1280 × 860, isolated Xvfb, the pointer parked at (600, 400) and
everything settled, then moved onto the rail and swept down it — nothing else
on screen moving, no scrolling, no clicks.

```
rest  vs hover  : 1981 px differ     ← the lens drew once, on entry
hover vs hover2 :    0 px            ← a one-pixel nudge changed nothing
sweep 200 -> 240:    0 px
sweep 240 -> 280:    0 px
sweep 280 -> 320:    0 px
sweep 320 -> 360:    0 px
sweep 360 -> 400:    0 px
sweep 400 -> 440:    0 px
sweep 440 -> 480:    0 px
```

Seven consecutive pixel-identical frames down the whole rail. The lens drew the
magnification for wherever the pointer *entered* and then held it.

**That is the "sometimes".** Entering the lane changes `mouse::Interaction`
(none → pointer), which forces a frame; so does a scroll, a tooltip, or any
other widget wanting one. The lens updates on the way past and looks like it
works — until you move within the rail and it does not.

## After

```
hover vs hover2 :  409 px differ     ← a one-pixel nudge now redraws
sweep 200 -> 240: 1363 px
sweep 240 -> 280: 1358 px
sweep 280 -> 320: 1031 px
sweep 320 -> 360:  856 px
sweep 360 -> 400: 1181 px
sweep 400 -> 440: 1488 px
sweep 440 -> 480: 1210 px
rest  vs left   :    0 px            ← the snap back is exact
```

`lens-at-e.png` is the strip at 3× with the pointer on `E`: the winner largest
under its wash chip, `D` and `F` next, tapering to rest size — the dock
mechanism the feature was asked for, visible for the first time since the 0.14
migration.

## The fix, and the one bit of state it cost

`Spine::update` now requests a redraw when the pointer **is in the lane, or was
in it at the last event**.

The second clause is the exit, and it is why the widget is no longer stateless:
without one more frame after the pointer leaves, the lens stays swollen at
wherever it left until something else wants a frame. That is the same defect in
its exit form, and it needs one `bool` in the widget's tree state to fix. The
module's claim to have *"no state at all"* is now qualified rather than
deleted, with the reason beside it.

Cost at rest is unchanged: a pointer that is neither in this lane nor was in it
last event asks for nothing, so a mouse crossing the wall costs exactly what it
did before.

## Both views, one widget

The owner named the Library and the Playlists views. They call one
`views::shelf::index_rail_from`, which builds one `Spine` — so this is one
defect seen twice, and one fix.
