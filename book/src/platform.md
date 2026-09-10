# Windows, dialogs, clipboard, cursors

Everything in this chapter lives in the windowing crate, and the pattern
is the same in all four cases: **the core learns nothing about it.**
That is not modesty, it is what keeps the `no_std` profile honest.

## Multi-window

`Host` is already one tree and one surface, which turns out to be the
right unit. A second window is a **second `Host`**, not a `Host` with two
of anything.

So multi-window is bookkeeping in the windowing crate — a map from window
id to host — and it needed **no core change at all**. It was one of the
two M15 capabilities that came in smaller than planned.

The practical consequence: two windows are two independent trees, two
independent stylesheets and two independent damage streams. They share
nothing except your application's state, and only if you arrange it.

## Clipboard

Copy and paste is split in half, deliberately:

* **The framework's half** — `Host::copy` (what is selected) and
  `Host::paste` (type this in). This is where the selection lives, so
  this is where it belongs.
* **The platform's half** — the actual system clipboard, in the
  windowing crate.

It is on by default: eleven crates, which the gating policy judged cheap
enough for something this close to table stakes.

## Dialogs

File, colour and font pickers, behind a **default-off** `dialogs`
feature — and the measurement is the reason rather than a formality.
`rfd` costs seventy-nine crates against clipboard's eleven and
accessibility's sixty-two (`docs/plan-capabilities.md` T15.11).

That is the gating policy working as designed: a capability whose cost is
large gets a switch, and the number is what decides which side the switch
defaults to. Nothing here is paid for by an application that does not ask.

## Cursors

The pointer shape is a **`Style` property**, not an event and not a
widget method — which means it cascades. A stylesheet says what the
pointer looks like over a kind or a class, and every node of that kind
gets it, including custom widgets that implement nothing.

It is also free in the sense the gating policy cares about: `Cursor` fits
in `Style`'s existing tail padding, which is asserted in
`crates/lumen-static/tests/cursor.rs` — because it would stop being free
if a future field pushed the struct over, and a byte in `Style` is a byte
times every node of every tree.

## The pattern worth taking away

Four capabilities, four different mechanisms, one rule:

| capability | mechanism | why |
|---|---|---|
| multi-window | none needed | a second `Host` was already the right shape |
| clipboard | default-on feature | small, and close to table stakes |
| dialogs | default-off feature | large dependency, measured |
| cursors | a `Style` field | cascades, and fits in existing padding |

The rule is that **none of it is paid for by an application that does not
ask** — and which mechanism enforces that is chosen per capability, from
the measurement, rather than by habit.
