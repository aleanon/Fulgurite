# Should a container's width change re-damage what did not move?

**Decided 2026-09-08 — `docs/plan-invalidation.md` T16.11.** The
framework half of a finding whose benchmark half was fixed in T16.2.

## The observation

`List::paint` clips its items to `rect_of(&st[i])` on its own node, and
that node's width is the **widest row**. A row whose text gets one pixel
wider therefore rewrites the clip, and retention's `Diffing::step`
unions the old ink with the new — so a one-row change damages the entire
column.

Measured on the frame arm at N = 100, before T16.2 pinned the width:

| frame | damaged |
|---|---:|
| a row whose width did not change | 1 562 px |
| a row whose width did change | 57 600 px |
| ratio | **37×** |

Because the arm reports a *minimum* over 200 iterations, it
preferentially selected the frames where the width happened not to
change, and the published `+render (damage)` figure described the lucky
ones. That is the half T16.2 fixed, by pinning the column so the number
describes every frame.

This document is about the other half: **is the framework right to
damage the whole column?**

## Why it currently does

`Recording::push_clip` observes `(hash, rect)` where the rect *is* the
clip. `Diffing::step` sees a changed hash and reports `old ∪ new`, which
for a clip spanning the column is the column.

That is the correct rule for every *other* command. A rect that moved
must repaint where it was and where it now is; a union is exactly right,
and it is why `damage_oracle.rs` holds.

## Why it is wrong for a clip specifically

A clip is not ink. It does not draw anything; it changes what the
commands *inside* it are allowed to draw. So the region a clip change
can affect is precisely where the clipping changed — the **symmetric
difference** of the old and new rects — and not their union.

Content strictly inside both the old and the new clip is unaffected by
the change: it was fully drawn before and is fully drawn now, and if its
own geometry changed, its own command reports that separately. Content
in the band between the two rects genuinely does change: it was cut and
is not, or was not and now is, and — importantly — **its own command
hash is unchanged**, so nothing else will report it. That is why the
clip must contribute *something* and cannot simply observe an empty
rect.

For a width-only change the symmetric difference is a vertical band
`[min(w₁,w₂), y₀, max(w₁,w₂), y₁]` — one strip, not the column. At the
N = 100 measurement above that is roughly 22 px wide by 800 tall against
a 72×800 column: about a third, and far less on a taller list where the
old and new widths differ by a pixel.

## The decision

**The rule is wrong and the fix is known, and it is not being built
now.**

Implementing it means `Diffing::step` distinguishing a clip from ink,
which means a kind reaching `Retention::observe` — and that seam is the
one M13/T13.8 shaped deliberately, to keep retention expressible on a
host with no display list. Widening it for a desktop damage optimisation
is exactly the trade T16.8 refused, and it should not be made casually
in a milestone whose other tasks did not need it.

The shape it should take when it is built:

- an additive `Retention::observe_clip(hash, rect)` defaulting to
  `observe`, so `Rebuild`, `Streaming` and any host strategy are
  unaffected until they opt in;
- `Diffing` overriding it to report the bounding box of the symmetric
  difference rather than the union;
- `damage_oracle.rs` extended with a case whose container width changes
  — which it currently has none of, and which is why this went unnoticed
  through M8b and M13.

**What makes it not urgent.** T16.2 pinned the benchmark's column, so no
published number depends on the amplification any more. And a real
application's list is usually the width of its window rather than of its
widest row, which is the case that never triggers this at all — a
`Length::Fill` or `Length::Fixed` container has a clip that does not
move. The amplification is specific to a `Shrink` container whose
content width is data-dependent, which is a real shape and not the
common one.

**What would make it urgent.** An application that scrolls or animates
inside a shrink-width container: there the clip moves every frame, the
damage is the whole container every frame, and damage tracking buys
nothing at exactly the moment it is supposed to be earning its keep.
That is the condition on which this should be reopened.

## What was checked and is not affected

- `damage_oracle.rs` still passes: its fixture's card is `Fixed(200)`,
  so its clip does not move, and the property it proves — a partial
  redraw equals a full one — is about correctness, not size. **The
  amplification never produced a wrong frame**, only a large one.
- T16.12's culling does not interact with this. Culling changes which
  commands are emitted; the clip is still emitted and still spans the
  container.
