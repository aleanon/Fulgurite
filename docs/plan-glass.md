# M18 — `Cmd::Glass`: backdrop blur, refraction and specular as one command

> **Status: planned 2026-09-10, not started.** Origin: a question about
> whether the renderer-scope answer ruled out blurred nodes and
> Apple-style "liquid glass". It does not, and the reason is worth a
> plan rather than a paragraph: **glass is closed-form over a signed
> distance field we already evaluate**, which puts it on the cheap side
> of the line `docs/results-vello.md` §4 draws, not the expensive one.

*Each task is one commit; each ends with `just test` green. Task ids go
in commit subjects. Every task states what it must be shown to fail at,
because a rendering feature whose tests have never been red is a
rendering feature with no tests.*

---

## 1. The decision this plan encodes

**Glass is one command, not a layer system.**

The material has four parts, and none of them is a path:

| part | needs | already have |
|---|---|---|
| backdrop blur | the destination pixels under the rect | they are in the buffer |
| tint and saturation lift | per-pixel colour arithmetic | `blend_px` |
| edge refraction | displace the sample by the SDF **gradient** | rounded-rect SDF |
| specular rim | `dot(gradient, light)`, sharpened | the same gradient |

This is the move `Cmd::Shadow` already is. Its doc comment records the
first version: eighteen translucent rounded rects per shadow, 97% of a
frame (`docs/results-frame-cost.md`). Collapsing that stack into one
closed-form command is what fixed it, and a filter graph for glass would
be the same mistake with a different silhouette.

**What this plan is not.** Group opacity is genuinely a layer — a
subtree rendered in isolation and composited — and it is *not* here. So
are arbitrary filters on arbitrary subtrees, `clip-path`, rotation and
everything else in `docs/results-vello.md` §4's third group.

## 2. Why the backdrop is the easy case and opacity is the hard one

CSS files `opacity` and `backdrop-filter` under the same heading and the
intuition that follows is wrong for this renderer.

`Surface::render` walks the command list **in order into one buffer**.
When a glass command is reached, everything behind it is already sitting
in `self.px` at those coordinates. There is nothing to render in
isolation and nothing to composite: the backdrop is the destination.

Group opacity has the opposite shape — the subtree must be rendered
*without* the backdrop, then composited at α — and that needs a real
offscreen target with its own lifetime. That is why one is in this plan
and one is not.

What glass does need is a **scratch buffer the size of the element's
source region**, because a blur cannot read and write the same pixels.
That is a transient the size of a panel, not a layer tree.

## 3. The command

```rust
Glass {
    rect: Rect,
    /// Corner radius, matching `RoundedRect`'s meaning.
    radius: f32,
    /// Backdrop blur, as a Gaussian standard deviation in px. 0 is a
    /// clear lens — refraction and specular with no blur, which is a
    /// real look and the cheapest way to test everything but §5.3.
    blur: f32,
    /// Multiplied over the blurred backdrop; `tint.3` is its strength.
    /// An opaque tint is a rounded rect with extra steps, and §8 says
    /// why no test may use one.
    tint: Color,
    /// How far, in px, the edge displaces what is seen through it.
    /// 0 is frosted glass; a few px is a lens.
    refraction: f32,
    /// Rim highlight strength, 0..=1.
    specular: f32,
    /// Direction the light comes from, radians clockwise from +x.
    light: f32,
}
```

**Budget: free.** `size_of::<Cmd>()` is 60 with align 4, set by
`Gradient`'s 56-byte payload plus the tag. `Glass` is 40. T18.0 asserts
`size_of::<Cmd>() == 60` the way `tests/cursor.rs` asserts `Style`'s 72,
so the next field added here has to argue for itself.

**The `Painter` default is a degraded draw, not a no-op**, following
`gradient`'s precedent — "a missing background is a worse failure than
an un-graded one":

```rust
fn glass(&mut self, rect: Rect, radius: f32, blur: f32, tint: Color, ..) {
    self.rounded_rect(rect, tint, radius);
}
```

Every existing painter — the checksum sink, the SVG backend, an
allocator-free embedded canvas — keeps compiling and draws a tinted
panel instead of nothing.

## 4. The maths

Let `p` be the pixel centre, `d = sd_round_box(p, rect, radius)`
(negative inside), and `n` the unit gradient of that field — for a
rounded box, `normalize(max(|p - c| - h, 0))` outside the inner rect and
the dominant axis within it. Both already exist: `shader.wgsl` has
`sd_round_box`, and `fill_rounded_within` computes the same distance
inline.

1. **Coverage.** `cov = clamp(0.5 - d, 0, 1)` — the identical ramp
   `fill_rounded_within` uses, so a glass panel and a rounded rect of
   the same geometry have the same silhouette to the pixel. T18.1
   asserts that against a rendered `RoundedRect`.
2. **Edge factor.** `t = clamp(-d / refraction, 0, 1)`, inverted:
   `e = (1 - t)²`. Zero in the middle, one at the rim.
3. **Refraction.** Sample the blurred backdrop at `p + n * refraction * e`,
   bilinear, clamped to the captured region.
4. **Tint.** `c = mix(backdrop, tint.rgb, tint.a)`.
5. **Specular.** `s = pow(max(dot(n, light_dir), 0), 32) * specular * e`,
   added as white. Confined to the rim by `e`, which is what stops it
   reading as a gradient overlay.
6. **Blend** `c` into the destination at `cov`.

**The blur is three box passes, not a Gaussian.** Box blur with running
sums is O(1) per pixel in the radius; three of them approximate a
Gaussian to within a few percent, and the box size is the one SVG's
`feGaussianBlur` specifies: `d = floor(σ · 3 · √(2π) / 4 + 0.5)`. Using
the spec's own approximation rather than inventing one means the result
is comparable with a browser's, which matters the first time someone
holds the two side by side.

At σ above about 4 the capture is downsampled by `k = clamp(⌈σ/2⌉, 1, 8)`
first, blurred at `σ/k`, and sampled back bilinearly. **This is the
whole performance story of the feature** and T18.3 measures it; nothing
above is a number and none of it has been measured yet.

## 5. The three costs, and the rule each one gets

### 5.1 It serialises

A glass command must see everything before it and nothing after, so it
cannot be reordered, batched or deferred. Fine for the handful a real
window has — a toolbar, a sheet, a sidebar. **Not** a general-purpose
primitive, and T18.7's `Style` plumbing must not make it feel like one.

### 5.2 It crosses band boundaries

`Surface`'s entire safety argument is that bands own disjoint rows. A
blur reads rows another worker owns, mid-flight. The rule:

> **The command list is segmented at glass boundaries.** Each segment is
> banded in parallel as today; between segments the workers join and the
> glass runs.

Capture and blur are serial. **The composite is not** — once the source
is in scratch, each output row reads only scratch and writes only its
own destination row, so the glass rect's rows parallelise by the same
`chunks_mut` argument the bands already use.

Cost: a frame with G glass commands has G+1 parallel segments and G
joins. That makes `std::thread::scope`'s per-render spawn G+1 times as
expensive, which is a second reason — the first is in
`docs/results-vello.md` §3 — to want a persistent pool.

### 5.3 Its pixels depend on pixels it does not own

This is the one that touches invalidation, and it is the reason T18.2
exists as its own task.

A glass panel over a scrolling list must repaint when the list scrolls,
even though its own command is byte-identical. Nothing in `Diffing`
knows that today: `observe(hash, ink)` says "this command drew here",
and a command whose hash did not change contributes no damage.

The new concept is a **dependency**: a region whose content is a
function of another region.

```rust
fn observe_dep(&mut self, src: Rect, dst: Rect);
```

`Recording::glass` reports `dst = rect` and
`src = rect` dilated by `3σ + refraction` — the furthest a pixel can be
read from. Then `Diffing::finish`, after computing damage `D`:

```
repeat until fixed point (capped at DEP_ROUNDS):
    for each (src, dst): if D ∩ src ≠ ∅ then D ∪= dst
```

The fixpoint is for glass over glass; the cap keeps a pathological scene
from looping and falls back to `Damage::All`, which is always correct
and merely slow.

**Deliberately conservative in one way, stated so nobody "fixes" it
later:** the rule uses all of the frame's damage, not only damage from
commands *earlier* in the list. A glass panel therefore repaints when
something in front of it changes too. Tracking order would need damage
attributed per command index, which is a larger change to `Diffing` than
this feature justifies, and the over-damage is one panel.

## 6. Tasks

**Ordering principle: every commit leaves the tree correct.** T18.1
introduces a command that reads the backdrop, which creates the
staleness bug in §5.3 — so T18.1 ships with the *conservative* rule
("a frame containing glass damages each glass rect unconditionally"),
which is correct and slow, and T18.2 replaces it with the dependency
test and measures what that bought. The oracle exists before the
optimisation, as in M16.

| id | task | done when |
|---|---|---|
| **T18.0** | The command, the `Painter` default, `hash_into`, `Scaled` forwarding, the `size_of::<Cmd>() == 60` assertion. No renderer implements it yet; every renderer draws the tinted rounded rect. | A glass command in a list renders as a tinted rounded rect on both backends, and `Cmd` has not grown. |
| **T18.1** | Software: coverage, refraction, specular, `blur = 0`. Backdrop read + bilinear sample from a scratch capture. Conservative damage rule. | Silhouette is pixel-identical to `RoundedRect` at the same geometry; a lens over a photograph displaces visibly and is stable frame to frame. |
| **T18.2** | Damage dependencies: `observe_dep`, the fixpoint in `finish`, `DEP_ROUNDS`, and the oracle test. Replaces T18.1's blanket rule. | Incremental and rebuild renders are byte-identical over a **moving** backdrop, and a still frame with glass damages nothing. |
| **T18.3** | The blur: capture dilation, downsample, three box passes, upsample. A results doc of its own — named on landing, per the rule that a `results-*.md` carries numbers — with the σ × panel-size sweep and the downsample threshold. | The sweep exists and the σ→cost curve is flat past the downsample threshold. |
| **T18.4** | Threading: segment the command list at glass boundaries; parallelise the composite over the glass's own rows. | `just threads`' byte-identity holds with glass in the scene, at every strip height. |
| **T18.5** | GPU: split the render pass at each glass command, blit the source region to a scratch texture, blur it, draw `KIND_GLASS` sampling it. | `just gpu-parity` holds within the existing 0.2% bound with glass in the scene. |
| **T18.6** | Embedded: re-measure `.text` and `.bss` against the ceilings; confirm the no-alloc profile takes the `Painter` default rather than growing a scratch buffer. | `just thumb` green with the ceilings restated, not raised silently. |
| **T18.7** | `Style.glass` so it cascades like `shadow` and `radius` do, plus one widget in the tally. Must re-assert `size_of::<Style>()`. | The tally has a glass toolbar over its scrolling content, and `Style` did not grow. |

**Sizing.** T18.0 and T18.1 are a day together. T18.2 is the interesting
one and should be budgeted as such — it changes `Retention`, which
`damage_oracle.rs` and `invalidation_oracle.rs` both pin. T18.3 is a day
including the sweep. T18.4–T18.6 are half a day each if 5.2's
segmentation lands cleanly. T18.7 is small and is where the feature
becomes visible.

## 7. What must be shown to fail

Per `AGENT.md`. Each row is a test and the injection that must turn it
red, named up front so the task cannot quietly ship without one.

| test | injection | what it proves |
|---|---|---|
| T18.1 silhouette | widen the coverage ramp to `1.0 - d` | glass and `RoundedRect` share one edge, not two that look alike |
| T18.2 damage oracle | delete the `observe_dep` call | the glass goes stale over a moving backdrop |
| T18.2 still frame | make the dependency unconditional | a still frame with glass stops being free |
| T18.3 blur | wrong box size for σ | the three-pass approximation is the specified one |
| T18.3 capture | shrink the source dilation to `rect` | the panel's rim samples clamped edge pixels and darkens |
| T18.4 threading | let a band composite glass without the join | a glass spanning bands reads half-written rows |
| T18.5 parity | skip the GPU blur pass | the two backends stop agreeing |

## 8. The two ways this plan will produce a vacuous test

Both are the documented failure shapes in `AGENT.md`, and both are
*specific to this feature*, which is why they are written down before
anyone writes the tests rather than after someone finds them.

**A static backdrop cannot exhibit a stale glass.** If T18.2's scene
does not move behind the panel, the incremental and rebuild renders
agree about a frame in which nothing happened — the invalidation
oracle's exact failure (T16.7), one feature later. **The backdrop must
move, and the test must assert that it moved** (a changed pixel count
under the panel), not only that the two renders match.

**An opaque tint hides everything this feature does.** With
`tint.3 == 255` the blur, the refraction and the backdrop are all
invisible, every injection above still produces the right pixels, and
the suite is a rounded-rect test wearing a costume. This is the GPU
damage gate's failure exactly — a half-cleared boundary pixel under
*opaque* content is overwritten and the artefact never survives to be
compared (T15.12) — and the fix is the same one: **every glass test uses
a translucent tint over content with structure in it**, and asserts the
backdrop is visible before asserting anything else.

## 9. What this does not buy

Stated so the plan is not read as a step toward general filters.

- **No group opacity.** §2.
- **No blur on arbitrary subtrees.** `Cmd::Glass` blurs what is behind a
  rounded rect. A `filter: blur` on a node blurs *the node*, which is a
  layer, which is §2 again.
- **No rotation, no `clip-path`, no SVG.** Unchanged by this plan;
  `docs/results-vello.md` §4 is still the map.
- **No claim about cost.** Nothing in this document is a measured
  number except `size_of::<Cmd>() == 60`. T18.3 produces the results
  doc, and until it does, "glass is cheap" is a hypothesis with an
  argument behind it and no evidence. This plan deliberately cites no
  such file yet: `scripts/check_doc_paths.sh` refused the forward
  reference, which is the gate working.
