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

> **Partly superseded 2026-09-10 by M18**, which made `Damage` a set of
> regions rather than a bounding box. That does **not** implement the
> `observe_clip` seam below — a clip is still observed as ink, and the
> symmetric-difference rule is still unbuilt — but it removes the half of
> the cost that came from *merging*: two damaged regions no longer become
> the box spanning them. Measured on the case M18's own test uses, a
> scrollbar thumb plus a status label, the box was **141×** the regions.
> The clip-versus-ink half remains open on the terms below.

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

## A second amplification, on the agent's side — M27/T27.3, 2026-09-19

This document is about what the **renderer** damages. The agent had its
own, larger, and it went unnoticed for the same reason the first did:
nothing asked a per-widget damage question until a drive script did.

**Measured on the showcase, live**, by `just widget-drive`:

| interaction | damage | the widget's own box | ratio |
|---|---:|---:|---:|
| click a `Button` | 691 200 px² | 4 059 px² | **170×** |
| click a `Switch` | 691 200 px² | 960 px² | **720×** |
| click a `CheckBox` | 691 200 px² | 576 px² | **1 200×** |
| **after T27.3, all three** | the widget's own rect | — | **1.00×** |

691 200 px² is 960 × 720 — the whole window, for every interaction.
Three causes, all in `lumen-static-agent`, none in the renderer:

1. **`journal::measure_into` folded a container's signature from its
   children's.** One leaf's repaint flipped every ancestor's hash, and
   every ancestor was then reported changed *with its own rect* — the
   root's being the window. The fold was a real optimisation (it
   replaced re-painting each subtree at every depth) and it solved the
   right problem; it just also made damage grow with depth. A container
   now signs what its own `paint` draws — shadow, fill, radius, border —
   which is four state reads and cheaper than what it replaced.
2. **The same fold meant a container's own paint was never signed at
   all**, so a container whose background, radius, shadow or border
   changed reported *nothing*. There was no test for it, because the
   fold made every container look busy whenever anything under it moved:
   **one defect hid the other.**
3. **`Cmd::hash_into` folded a text run's offset into the shared text
   buffer.** `measure_into` paints every node into one list, so a node
   whose text changed *length* shifted the offset of every node painted
   after it, and all of them reported a repaint they had not made. In
   the showcase the readout going from `checked: false` to
   `checked: true` — one byte shorter — reported the 954 px note at the
   far end of the window as repainted: **61 056 px² of phantom damage**
   for a click on a 24 × 24 checkbox.

And a fourth, in the surface rather than the measurement:
**`ui.lastDamage` reported the bounding box of every changed node**,
which is the union M18 replaced. M18 changed the renderer and this
surface was never updated, so the agent kept answering the pre-M18
question — `regions` now reports the set, and `rect` is documented as
the weaker answer.

`crates/lumen-static-agent/tests/damage_shape.rs` holds all four, and
**two of its five tests were themselves vacuous on the first attempt**,
each proved so by the injection aimed at it: a sibling that paints a
rect cannot notice a shifted *text* offset, and `summed <= hull` is an
inequality a bounding box satisfies exactly. Both were strengthened, not
softened — the second now asserts the region *count*, which is the only
thing that separates a set from its hull.

The renderer's 37× above is unaffected by any of this and remains as
recorded.

**What the 1.00× measures, and what it does not** (M40/T40.1,
2026-09-25). `ui.lastDamage` is derived from the agent's own journal of
`NodeState` geometry, which is **logical**, and it never reads
retention; the drive harness divides it by a logical widget box. So the
ratio is *journal damage in logical pixels* — which node the agent saw
change — and **not the damage a surface presents**, which since T40.1 is
retention's, in **physical** pixels. The two agree at a scale of 1.0 and
nowhere else. Before T40.1 they disagreed in the worse direction: at 1.5
the journal reported the hovered button's own box while the surface
redrew a logical-sized rect at the logical position and left the
physical rect partly stale (R1-F1, `docs/plan-physical-damage.md` §2.1),
and the 1.00× could not see it. T40.1 did not change `ui.lastDamage`;
it changed `app.perf`'s `damaged_px`, the one agent-visible quantity
counted from the presented regions, which grows by the scale squared
because it had been counting logical rects as physical pixels. Reporting
the presented damage beside the journal's is M48's (T48.3, T48.4).

**And it holds only for repainting actions, and now for every one of
them** (M48/T48.4, 2026-09-26). The drive skipped its bound when no
region overlapped the widget, so an action that left *no* damage — the
journal stale, or nothing painted — passed silently; deleting the
window's redraw after an agent request left `just widget-drive` at exit
0. An action now declares whether it repaints (default: unless it is
`static`), a repainting action with no region over any of its nodes
fails, and each damage line prints `presented P px` beside the ratio —
what the surface presented over the action's journal frames, from
`app.perf`'s `since`. The first run under that rule found three things
the skipped check had hidden: the showcase's checkbox, switch and radio
never showed the flag they toggled (a presentation copy nothing wrote
back), the journal never signed a `Picker`'s trigger (a part its owner
paints), and a caret's animation could push an action past the
journal's 64-frame history. All three are fixed in the same commit.


## A third: a command-count change was the whole window — M26/T26.11, 2026-09-19

`Incremental` compared its retained slots **slot-by-slot during the
walk**, overwriting each as it went. So when `finish` learned the two
frames had different lengths, the previous frame no longer existed and
the only honest answer was `Damage::All`. Every popup open and close,
and every enum-variant switch, repainted the window — on the strategy
the desktop shell defaults to.

It keeps two buffers now and aligns them in `finish`: longest common
prefix and suffix by hash, damage the middle. Nothing guesses — a prefix
matches only where the hashes are equal — so a stream that genuinely
changed everywhere still reports rects covering the viewport, which is
the same answer as before arrived at by measurement.

| | before | after |
|---|---:|---:|
| a popup over unchanged chrome (`tests/retain_align.rs`) | the viewport, 120,000 px² | **3,200 px² — the popup's own rect**, 37.5× less |
| the showcase's real dropdown (`just profile-showcase`) | 691,200 px, **100.0%** | 580,680 px, **84.0%** |

The showcase's own frame improves by much less than the synthetic one,
and that is honest rather than disappointing: opening its dropdown also
changes the trigger's label and re-lays the stage, so a lot of that 84%
genuinely did change. The synthetic arm isolates the property; the
showcase arm prices it on a real screen.

**`Streaming<N>` does not get the second buffer**, deliberately. It
would take `Streaming<256>` from 6,680 B to 13 KB — a tenth of the M13
device's RAM — for a strategy whose whole purpose is a bounded,
allocation-free footprint. A length change still answers `Damage::All`
there.

And the per-command price *fell*, because the comparison left the hot
path: `--bin retain`'s `streaming: build + Incremental` arm, 34
commands, **328 → 295 ns**.
