# M22 — the complete liquid material

> **Status: planned 2026-09-11, rewritten the same day after a
> four-agent review.** Seven tasks, where the first draft had five.
>
> The review found **five of the first draft's seven injections would
> have passed under their own injection**, which is the failure
> `AGENT.md` exists to prevent, occurring in the section written to
> prevent it. It also found the headline feature could not work as
> specified, that the fallback for the hardest task could not be built
> where the plan put it, and a cheaper effect the plan had not
> considered. Everything below is the second draft; §11 records what the
> first got wrong, because the corrections are more useful than the
> plan.
>
> Builds on M19 (`docs/plan-glass.md`) and M21's bevel
> (`docs/results-glass.md` §5b). What exists: backdrop blur, edge
> refraction over a spherical bevel, one specular lobe, a tint.

*Each task is one commit; each ends with `just test` green. Task ids go
in commit subjects. Every task names the injection its tests must go red
under, and §9 names the vacuous test each will otherwise produce.*

---

## 1. The budget, and the encoding decision it forces

Measured 2026-09-11 (and **the numbers belong in a results doc, not
here** — M22's first commit moves this table to `docs/results-glass.md`
and cites it, per `AGENT.md`'s "stated once"):

| | bytes |
|---|---:|
| `Material` | 24 |
| `Cmd::Glass` payload (`rect` 16 + `radius` 4 + `Material`) | 44 |
| `Gradient`'s payload — **what sets `Cmd`'s size** | 56 |
| `Cmd` | 60 |

Headroom is 12 bytes. The first draft spent it in `f32` and ran out
before the morph; measured, a fourth `f32` takes `Cmd` to 64.

**Every strength this milestone adds is a unitless 0..=1 fraction, and
every one is a `u8`.** The repository already made this call and wrote
down why: `Style::focus_ring` is a `u8` "and that was measured. As an
`f32` this took `Style` from 72 to 76 bytes." Measured for `Material`:

| encoding | `Material` | `Cmd` | left |
|---|---:|---:|---:|
| four `f32` strengths | 40 | **64** | — |
| four `u8` strengths | 28 | 60 | **8 B** |
| four `u8` + a second tint `Color` | 32 | 60 | **4 B** |

So the whole milestone fits, and the morph's remaining 4 bytes is the
constraint T22.5 designs inside — §7.

**What each task spends:** T22.0 two `u8` (`shadow`, `saturation`) and
**zero** for Fresnel, which is derived from `specular`. T22.2 one `u8`
(`dispersion`). T22.3 one `u8` (`adapt`) and one `Color`. T22.4 nothing.
T22.5 the remaining 4 bytes, or a re-budget.

**Three hand-written lists must learn every new field**, and two of them
fail *open*: `Recording`'s hash array and `Cmd::hash_into`'s are literal
field lists in different crates — the exact shape that let `Shadow` and
`Image` both hash as tag 6 for a year. **Omitting a field from the first
is silent staleness**, not a wrong colour: the hash is unchanged, damage
reports `None`, the shell skips the raster, and the panel keeps last
frame's pixels indefinitely. §8 has a row for it and §9 explains why the
existing test cannot catch it.

`Scaled::glass` is the third, and it fails open only for a **length**:
its `..material` rest pattern silently does the right thing for every
unitless field and the wrong thing for a new distance. Nothing in
T22.0–T22.4 adds a length. T22.5's offset does.

## 2. T22.0 — Fresnel, inner shadow, saturation

The first draft led with dispersion. The review's strongest optical
finding is that **Fresnel is the bigger effect and costs nothing**, so
it leads instead.

### Fresnel — why the rim is bright

Real glass reflects far more at grazing incidence, and that is what
makes a thick edge read as a bright band rather than as a tinted one.
The tint currently mixes at a constant strength everywhere. With the
viewer on the +z axis, the cosine of the incidence angle **is the
bevel normal's z component**, which is already in a register:

```
f  = 1 − nz
t' = t + (1 − t) · fresnel · f⁵          (Schlick)
```

Five multiplies and a subtract, in the rim band only, **and no new
`Material` field**: `fresnel` is derived from `specular`, which already
means "how glassy is this edge". A panel with no specular has no
Fresnel, which is the right coupling.

### Inner shadow — and the two claims the first draft got wrong

The first draft said the dot product "is already in hand… it is free".
It is not. It sits inside `if m.specular > 0.0`, and the bevel itself is
gated on `rim > 0 && (refraction > 0 || specular > 0)`. **A material
with only a shadow gets no dot product and no bevel at all** — a panel
compared with itself, and every injection green. Both predicates learn
the new field.

The second wrong claim was the falloff. Measured from the code: with the
light's elevation fixed at 0.62, the dot product is negative **exactly
when `u > 0.62`**, independent of light angle — the outer 38% of the
rim, 4.9 px of a 13 px bevel. "Darken the negative half" is a dark
hairline, not thickness.

**Use a diffuse term riding the squared bevel parameter, the same one the refraction
uses**, so the shading covers the whole unlit rim and falls to zero in
the interior. It needs the two horizontal terms the existing dot product
already computes.

**And it multiplies, it does not subtract.** The composite adds the
specular with no lower clamp and relies on a saturating cast. Subtracting
clips each channel at a different point, which rotates hue: a backdrop
(30, 60, 220) minus 200 is (0, 0, 20), and the ratios are gone.
Multiplicative darkening cannot go below zero and preserves them — which
also protects the saturation boost from being undone by the shadow in
the same pixel.

### Saturation

A blur that desaturates looks like fog; colours read *through* real
glass because the material does not eat them.

Two things the first draft left unstated, both of which decide the
result:

- **The weights.** The renderer blurs in straight, gamma-encoded 8-bit
  sRGB and never linearises. Rec.709's coefficients are *linear-light*
  luminance; applied to gamma bytes they over-darken blue and
  over-brighten green. On gamma-encoded values the consistent choice is
  the luma convention — 0.299, 0.587, 0.114 — and the field's doc says
  so, with the reason.
- **Out of gamut on both ends.** `lum + (c − lum)·s` exceeds 255 on the
  dominant channel *and* goes negative on the weakest. Clamping per
  channel rotates hue. **Clamp the scale, not the channels**: reduce `s`
  until every channel fits, so an over-saturated pixel desaturates
  gracefully. Guard it behind a cheap out-of-range test, because
  saturation applies to the **whole panel** and not the rim band — it is
  the one effect here whose cost is per-panel-area.

**The colour space goes in the field's doc comment**, because a fragment
shader's natural habitat is linear and a linear-vs-gamma saturation
disagrees over the whole panel — which is precisely what T22.6's parity
gate cannot absorb.

## 3. T22.1 — sub-pixel refraction sampling

A refactor with no new field, landed on its own because **it moves
existing pixels**.

The refracted read truncates to a whole surface pixel before sampling,
and the sampler only interpolates when the downsample factor exceeds 1 —
which at the liquid blur it does not. So today's refraction is
nearest-neighbour and its rim carries a staircase.

Taking float coordinates removes the staircase and is a prerequisite for
T22.2: a sub-pixel colour fringe cannot exist on an integer read.

**The pixel delta is the deliverable.** This task's results row is the
differing-pixel count against M21's output at `LIQUID`, stated rather
than absorbed, because every later task and T22.6's parity baseline
inherit it.

## 4. T22.2 — dispersion

Now that the read is sub-pixel, the three channels can be displaced
differently:

```
k = refraction · u²
r at p + n·k·(1 − dispersion)
g at p + n·k
b at p + n·k·(1 + dispersion)
```

**Blue deviates more, not red** — the first draft had it inverted, and
the ordering is the thing an eye reads as "lens" rather than colour
noise.

Three things the first draft missed:

- **`glass_source` must widen** to `3σ + refraction·(1 + dispersion) +
  4`. It is the one expression the renderer and the damage recorder
  share, and the existing `+ 4` is documented slack for a different
  purpose. Spending it silently is the drift the doc-currency rule
  targets — and the symptom is invisible to every gate: the sampler
  clamps, so the red fringe flattens against the capture edge
  *identically in the full and the partial render*.

  **Built 2026-09-12, and the slack is worse than this paragraph
  thought.** It does not merely hide the term from a reader; it hides
  it from a *test*. At `LIQUID`'s 11 px refraction the widening is
  2.75 px and the slack is 4, so removing the term changes nothing a
  fixture built on `LIQUID` can observe — the first gate measured 0 of
  240 and was green either way. The gate ships at refraction 100.
  `docs/results-glass.md` §0d.
- **The cost is per channel, not per sample.** The sampler computes one
  coordinate and returns three channels from twelve loads; calling it
  three times triples the coordinate setup and triples the loads. A
  per-channel entry point gives the plan's stated cost. And the interior
  must branch out, or it pays three times for three identical answers.
- **Per-channel clamping manufactures colour.** Where the capture was
  truncated by the surface edge, three displacements clamp at three
  different distances and an achromatic backdrop grows a coloured band.
  Fall back to the single three-channel read whenever the green
  coordinate is already clamped — one comparison, and it makes the
  degenerate case bit-identical to the pre-T22.2 renderer.

  **Corrected 2026-09-12: the green coordinate is the wrong one to
  test.** Green is the *midpoint*; blue can clamp while green is still
  inside, which is exactly the one-sided band the guard exists to stop.
  Guarding on green alone invents the same 35 levels of chroma from a
  grey ramp as having no guard at all — both measured. The shipped
  guard tests the extremes. `docs/results-glass.md` §0d.

## 5. T22.3 — adaptive tint, stateless

A fixed tint is legible over the backdrop it was chosen for. The
material should go light over dark content and dark over light, so what
is *on* the glass stays readable as content scrolls under it.

**It is a pure function of the captured buffer. There is no state, and
the first draft's fallback could not have been built.** The reasons are
worth stating because they are not obvious:

- The renderer has no path back to application state, and
  `render_within` is called **once per damaged region** — so a
  renderer-side update would run a variable number of times per frame,
  which is the one thing a damped value must not depend on.
- A renderer-computed mean never reaches the hash. On a frame where the
  backdrop did not change the hash is identical, damage is `None`, and
  the shell may skip the raster entirely — so a damped value advances
  only on frames something *else* changed, then jumps by the whole
  accumulated delta.
- And the oracles would not have caught it. They drive **one** host and
  render **one** list two ways; widget state cannot diverge between the
  sides. **The gate would have stayed green**, which is worse than
  failing.

So the flicker must be settled by smoothing, and this task carries a
**pre-registered exit**: if the measured frame-to-frame movement through
the threshold region exceeds a stated bound after smoothing, stop and
reopen the question rather than reaching for state. The bound and the
measurement go in `docs/results-glass.md` either way.

**Reduce over the panel's own rect — never over the clip.** The first
draft named the wrong hazard. Reducing over the dilated capture gives a
wrong colour *in both renders equally*, so the gates stay green.
Reducing over the clip — which is the damage region in a partial redraw
— makes the full and partial renders compute different means and differ
on **every pixel of the panel**. That is the central gate, and §8 has
the injection for it. It is safe today only because two rules in other
files happen to make the clip contain the panel; this task pins it.

One measurement this task owes before it trusts its input: the box
passes renormalise at the capture boundary, so the blurred mean over a
*sub-region* is not exactly the true mean. The error goes in the results
doc beside the bound.

## 6. T22.4 — press deformation

A glass button should squash: the rim thickens and the lensing deepens
under the finger.

**No new `Material` field and no renderer change.** The widget reads the
pressed state and modulates the material it assembles. Three corrections
to the first draft, each of which would have shipped a test that passed
on a panel compared with itself:

- **The pressed flag is set on the hit node only** — one node, the
  deepest, not the chain. For `Glass<Button>` the *button* is marked and
  the panel never is. The widget therefore tests **its whole subtree**,
  which matches the composition this task is about and needs no change
  to the dispatch. `NODE_COUNT` is a const, so the test is a slice scan.
- **The animation hook the first draft named does not reach the
  widget.** `App::tick`'s rustdoc records a decision: a per-node tick
  "would cost a virtual call per node per frame on trees that animate
  nothing — 3 000 of them on the benchmark's list — which the M15 gating
  policy forbids." **Keep that decision.** The widget *retargets* in
  `layout`, the one method holding both `&mut self` and the state array;
  the application *advances* it from `App::tick`. Ugly to write, and it
  is what makes widget animation reachable at all without overturning a
  measured design record.
- **And the frame loop must be fixed, or it snaps instead of settling.**
  The windowing crate ticks *before* it pumps, so a press that starts an
  animation during the pump leaves the "still animating" answer already
  false: no further redraw is requested, and the rim jumps to its next
  value on the following unrelated event. This task owns the reorder.
  The "done when" must be driven through the real redraw path, because a
  hand-rolled tick loop passes either way.

**This is `Animated<T>`'s first user in the workspace.** Nothing outside
its own tests uses it, so anything it does not already support is this
task's to find.

## 7. T22.5 — the morph

The "liquid" in the name is panels stretching and merging like droplets.
It looks like it needs arbitrary shapes and does not: two merging blobs
is a **smooth minimum** of two signed distance fields, which the first
draft named and never wrote down:

```
h    = clamp(0.5 + 0.5·(b − a)/κ, 0, 1)
smin = b + (a − b)·h − κ·h·(1 − h)
```

C¹ everywhere, exactly `min` beyond κ, and it undershoots by κ/4 where
the fields meet. κ ties to `rim`, so the neck's scale matches the
bevel's.

Five consequences. The first draft listed three and got two of them
wrong.

- **The bounding rect must become the union of both blobs.** The
  command's rect is the *ink*, the *clip test*, and the *source*. A
  second blob outside it is a stale panel (damage never covers it), a
  **visible hole** (a damage region covering only the second blob fails
  the clip test and the region was already cleared), and a short
  dependency. The individual shapes are then derived from the union.
- **It reports one dependency, not two.** `MAX_DEPS`'s rustdoc states
  "each panel contributes at most one region", and two morphing panels
  reporting two each would exhaust it and degrade the frame to a full
  redraw — correct, and a silent cliff. One dep over the union
  preserves the invariant; anything else corrects that comment in the
  same commit.
- **The blended normal is exact, and it is not unit.** Weighting the two
  gradients by the same `h` the value uses is not an approximation — the
  remaining term vanishes identically. But the blend's length is below 1
  wherever the gradients disagree and zero where they oppose, and the
  bevel is built on the normal being unit **by construction**. That
  claim becomes false at the neck, which is where it matters most. It
  must be normalised, with the epsilon guard the existing code uses.
- **A smooth minimum is not a distance field**, so the rim band pinches
  in the neck: the gradient's length is below 1 there, so `u` reaches
  zero after fewer pixels than `rim`. Dividing the distance by that
  length restores it, and costs nothing — it shares the square root the
  normalisation already needs. Without it the neck's bevel visibly
  narrows and the blame lands on the crease.
- **Coverage, and the invariant it threatens.** The first draft said
  per-pixel coverage is "a real cost increase"; it is not, because every
  pixel already gets a distance whenever the bevel is on. What it
  actually costs is **M19's stated silhouette parity**: `plan-glass.md`
  §4.1 records that glass keeps `fill_rounded_within`'s hard-span
  structure precisely so a panel is pixel-identical to a rounded rect of
  the same geometry, and that a pure SDF ramp disagrees by up to 254/255
  on a fractional edge. **Take the SDF path only when a second blob is
  present** — a branch on the command, not on the pixel — and assert a
  single panel is byte-identical before and after. Otherwise the whole
  M19/M21 baseline moves inside a commit whose subject is a morph.

**The budget.** After T22.0–T22.3 there are 4 bytes left (§1). An offset
and a uniform scale as `f32` is 12 and does not fit; as fixed-point
16-bit values it is 6, and `Material` lands at 36 with `Cmd` still 60.
The alternative is a second command variant and a re-budget — measured,
that is `Cmd` at 80, **+33% on every command in every list**, which
against a 2 000-command frame is 40 KB. Either way the decision is this
task's first act, it is a measurement, and it goes in
`docs/results-glass.md` — **not** `results-embedded.md`, which budgets
`.text` and per-node RAM and where a command size appears nowhere.
Boxing is not available: the command is `Copy` and the list is the
`no_std` seam.

## 8. T22.6 — the GPU backend, and why it is last

**The only task that changes what the material can be used for.**
Measured, `docs/results-glass.md` §2: ~21 ns/px — 0.6 ms for a toolbar,
**43 ms at 1080p**, 174 ms at 4K. A panel is affordable and a screen is
not.

The port is specified in `docs/plan-glass.md` §1.3. Last, because every
task above changes the material the shader must match, and T22.5 may
change the command encoding as well.

**The parity gate needs two bounds, pre-registered, and the scene needs
teeth.** As it stands the gate would pass with the feature absent:
today's parity frame has no glass at all; if the panel added were the
frosted preset the bevel would be off and skipping the shader's
dispersion would be a no-op on both sides; the comparison ignores
per-pixel deltas under 8 levels, which a fringe over a smooth backdrop
does not exceed; and "a bound stated with the measurement beside it"
lets the bound be chosen *after* seeing the number — which is exactly
how a 6% bound over a 0.01% measurement got written.

So: the scene uses the liquid preset with dispersion and adapt
explicitly non-zero over a **hard, saturated colour edge** under the
rim; the CPU side asserts the fringe exists *before* the backends are
compared; and there are two bounds — a whole-panel count, which the blur
disagreement forces loose, and a **separate rim-band bound tight enough
that a missing fringe cannot hide under it**. One frame-wide count
cannot both admit the blur and refuse the dispersion.

## 9. Tasks

| id | task | done when |
|---|---|---|
| **T22.0** | Fresnel (no field), inner shadow, saturation. Two `u8` fields. | Fringe-free: rim luminance at grazing incidence rises by a measured factor; the unlit rim is darker **than the panel's own interior** with specular off; a mid-chroma backdrop's chroma ratio through the panel exceeds a stated bound. All three printed. `Cmd` still 60 B. |
| **T22.1** | Sub-pixel refraction sampling. No field. | The staircase is gone, and **the differing-pixel count against M21 is the deliverable**, stated in the results doc. |
| **T22.2** | Dispersion. One `u8`. Widened source expression. | **Done 2026-09-12.** Over a hard edge the rim's blue − red is **+27.3** signed — signed because the *ordering* is the claim — against **0.00** chroma with dispersion off, both printed. Cost +10%, flat across panel size. `docs/results-glass.md` §0d. |
| **T22.3** | Adaptive tint, stateless, with the pre-registered exit. One `u8` + one `Color`. | A fine sweep of backdrop luminance moves the tint **strictly** monotonically with a stated total travel; a scroll through the threshold bounds the per-frame movement; and the sweep asserts the threshold was actually crossed. |
| **T22.4** | Press deformation, and the redraw-loop reorder. No field. | Driven through the real redraw path: pressing thickens the rim band by a measured pixel count at the animation's peak, and the settled frame is **byte-identical** to the pre-press frame. |
| **T22.5** | The morph. Budget decision first, with its measurement. | At a separation of one rim width the pixels between the blobs are inked; at three rim widths they are empty; and a single panel is byte-identical to before the task. |
| **T22.6** | The GPU backend. | `just gpu-parity` holds under **two** bounds, both stated before the measurement. |

**Sizing — in measurement cycles, not days.** The first draft said "a
day" per task; for calibration, M19's six tasks were 37 minutes of wall
clock and M21 was 85. What actually paces work here is the number of
measured rows a task owes `docs/results-glass.md`: T22.0 three plus a
cost delta, T22.1 one, T22.2 one plus a cost delta, T22.3 two (the
sweep and the mean's error), T22.5 two (the budget decision and the
coverage cost), T22.6 the parity bounds. T22.6's "2–4 days" is inherited
from `plan-glass.md` §1.3 rather than re-derived, and it is the only
task nobody here has attempted.

## 10. What must be shown to fail

Rewritten. Five of the first draft's seven rows would have passed under
their own injection; each is replaced with one that fires, and the
reason the old one did not is in §11.

| test | injection | fires because |
|---|---|---|
| Fresnel | fix the incidence term at a constant | the rim's grazing brightness collapses to the interior's |
| inner shadow | leave the bevel predicate ignorant of the shadow field | a shadow-only material renders with no bevel at all, so the unlit rim equals the interior |
| inner shadow, second | subtract instead of multiply | a saturated backdrop's channel ratios move — assert the hue, not the luminance |
| saturation | pass the backdrop through unchanged | the chroma ratio falls to 1.0 on a **mid-chroma** backdrop, where neither end clamps |
| sub-pixel sampling | truncate the coordinate again | the staircase returns; the count against M21's baseline is the measure |
| dispersion | give all three channels the same displacement | red and blue stop differing in the outer rim |
| dispersion, source | leave the source expression at its old reach | the outer ring differs between a full and a partial redraw |
| adaptive tint | freeze the mean | the sweep's **total travel** collapses to zero — monotonicity alone accepts a constant |
| adaptive tint, region | reduce over the clip instead of the panel | the damage oracle goes red: full and partial renders compute different means |
| press | ignore the pressed state | the rim-band difference between rest and peak collapses — **on a liquid fixture**, since a frosted one has no bevel to deform |
| press, settle | never return to the resting value | the settled frame stops being byte-identical to the pre-press frame |
| morph | plain minimum instead of the smooth one | the pixels between the blobs are empty at one rim width |
| morph, normal | use one blob's gradient instead of the blend | the luminance profile across the neck gains a step — bound the largest adjacent jump |
| morph, parity | take the SDF coverage path for single panels too | a lone panel stops being byte-identical to before the task |
| the hash | drop the new field from the recorder's list | a frame in which only that field changed reports no damage |
| neutral defaults | give a new field a non-neutral default | M21's scene stops being byte-identical with the new fields zeroed |
| GPU parity | skip the shader's dispersion | the **rim-band** bound fires; the frame-wide count would not |

## 11. The vacuous tests this will otherwise produce

Ten, where the first draft had five. Every one is a case where the
feature is invisible on the fixture and every injection passes.

**The five the first draft had**, corrected where it was too loose:

- **Dispersion and saturation over a grey backdrop split nothing.** And
  the subtler half the first draft missed: a box blur is a *mean*, so a
  striped or checked fixture — which is what the existing bevel tests
  use — averages to near-neutral before the effect sees it. The fixture
  must be a large solid colour region, and the *blurred* chroma is what
  the test measures.
- **Adaptive tint over a uniform backdrop crosses nothing**, and
  "monotonic" accepts a constant. Assert strict movement and a total
  travel.
- **Press with the flag never set** compares a panel with itself — and
  the first draft's guard ("assert the flag changed") is satisfied by
  the *button's* flag while the panel's stays false. Assert the flag on
  the node the material reads.
- **A morph at either extreme is not a morph** — and the first draft
  framed both extremes as vacuous when one is a required negative case:
  beyond the blend radius the blobs must read as **separate**, or the
  smooth minimum is fusing panels that should not fuse. Both directions,
  same test.

**The five it missed:**

- **A frosted fixture has no bevel.** The default material's rim is
  zero, so the bevel is off entirely and every rim-band effect — Fresnel,
  shadow, dispersion, press — renders a panel compared with itself.
  Every test of those effects builds on the liquid preset **and asserts
  the rim is non-zero before measuring**.
- **The press is temporal and the obvious test is one frame.** Nothing
  advances an animation unless the test advances it, so a test that
  presses and renders sees the value at rest: the flag changed, the
  pixels match, the injection is green. The test is a sequence with the
  animated value printed at each step.
- **Saturation clamps at the top end too.** A fully saturated backdrop —
  the obvious choice for "a saturated backdrop" — cannot be boosted, so
  "pass it through unchanged" leaves the pixels identical. Mid-chroma
  fixture, and assert no sampled channel sits at 0 or 255.
- **A non-neutral default silently re-baselines every existing test.**
  Nothing today asserts that a new field is inert at its default, and
  two public presets are about to gain four fields. One test per task:
  M21's scene with the new fields zeroed must be byte-identical.
- **A threshold sampled at round numbers is not sampled.** `plan-glass.md`
  §9 records this exactly — eight round sigmas passed with the search
  replaced by the rule it exists to beat. A flicker check at luminance
  0/64/128/192/255 steps straight over a threshold at the midpoint. Sweep
  finely through the threshold region, and sweep the morph's separation
  rather than testing one.

## 12. What the first draft got wrong

Kept because the corrections are the useful part, and because a plan
that quietly replaced itself would teach the next reader nothing.

| claim | what the review found |
|---|---|
| "Three bilinear samples instead of one" | The read is integer-truncated and the sampler does not interpolate at the liquid blur. There was nothing bilinear to take three of, and dispersion could not have produced a fringe — only a dithered speckle. |
| red displaced most | Blue deviates more. One character, and it flips which side the warm fringe lands on. |
| "the dot product is already in hand… it is free" | It is inside a specular guard, and the bevel predicate would have excluded a shadow-only material entirely. |
| "a rim lit on one side and shaded on the other" | The negative half exists only over the outer 38% of the rim: a hairline, not thickness. |
| "temporal damping… would live on the widget" | Cannot be built there, and — worse — the oracle would not have noticed. |
| "the rebuild has no history, so it diverges" | Both oracles drive one host and render one list two ways. State cannot diverge between the sides. |
| "every pixel needs a distance… a real cost increase" | Every pixel already gets one. The real cost is M19's silhouette-parity invariant. |
| "the two gradients weighted by which one dominates" | Correct, and exactly so — but the blend is **not unit**, which the bevel assumes. |
| "an offset and a scale — three `f32`s, which fits exactly" | The headroom was already spent. Measured, that is `Cmd` at 72. |
| "re-budgeted in `docs/results-embedded.md`" | Wrong document: it budgets `.text` and per-node RAM, not command size. |
| "`light` is a field and an application can already animate it" | Only the azimuth is. The elevation is a hardcoded constant, and azimuth rotates a highlight *around* a panel — it cannot sweep it *across* the rim, which is the motion a tilt produces. |
| sizing in days | M19 was 37 minutes of wall clock; M21 was 85. |

## 13. What this does not buy

- **Not Apple's, even complete.** No environment reflection from a real
  cubemap, and no light elevation — which is what a tilt actually
  changes, and which is one `u8` if it is ever wanted.
- **A second, broader specular lobe is two arithmetic operations from
  values already computed**, and is declined for *look*, not cost. Said
  plainly because both this plan and the results doc previously implied
  it was expensive.
- **Not full-screen, until T22.6.** §8.
- **No measured cost yet.** Only §1's byte counts and §8's 21 ns/px,
  both of which move to `docs/results-glass.md` in T22.0. Every task
  adds its own rows; §9 says which.
