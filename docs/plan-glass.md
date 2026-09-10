# M19 — `Glass<W>`: a backdrop filter as a widget

> **Status: planned 2026-09-10, not started. Blocked on M18 (§2).**
> Its other prerequisite, `plan-widgets.md` T11.6's `stack` axis (§2.1),
> **landed 2026-09-10**.
>
> Rewritten from the first draft after a four-agent spec review found
> ~35 defects in it, two of which were in shipped code and are already
> fixed (`1de77c6`). This version records the decisions that review
> forced, and is deliberately much smaller than the plan it replaces.

*Each task is one commit; each ends with `just test` green. Task ids go
in commit subjects. Every task names the injection its tests must go red
under, because a rendering feature whose tests have never failed has no
tests.*

---

## 1. The five decisions

The first draft got all five wrong or left them open. They are settled
here so the tasks do not re-argue them.

| | decided |
|---|---|
| **Where the parameters live** | On a widget, `Glass<W>`, as ordinary struct fields. **Not** on `Style`. |
| **What ships first** | Blur only. Refraction and specular are a later, purely additive task. |
| **Which backend** | Software. The GPU backend draws the degraded fallback until a later milestone. |
| **When** | After `Damage` carries a set of regions (M18) and a container can overlap its children (T11.6). Not before. |
| **How it stays cheap** | A backdrop cache per panel, so a recompute is bounded by the panel and not by the frame. |

### 1.1 Why a widget and not a `Style` field

Two reasons, and the second is the one that decides it.

**Bytes.** `Style` is 72 with zero spare — the tail slot `cursor` and
`focus_ring` were fitted into is full. Measured 2026-09-10
(`docs/results-embedded.md`): a full glass parameter set is +20 B, taking
`Style` to 92 and per-node RAM from 148 to 168 B. That is +236 B of
`.text` and no measurable frame cost, so it is *cheap* — but it is paid
by every node of every tree, including the embedded profiles that take
the `Painter` fallback and cannot render a blur at all. `Glass<W>`
carrying its own fields costs **zero** on every tree that does not use
one, and gives per-instance tuning that a packed `u8` preset index could
not.

**Paint order, which a style property cannot express.** Glass reads what
was painted *before* it. A `Style` flag is applied wherever the derive
emits it, which is not a moment the author chose. A widget occupies a
position in the tree, and that position *is* the answer to "what does
this blur". `ScrollView<W>` and `Popup<W>` already have exactly this
shape, and `Popup` already solves the harder half with `out.defer(i)`.

**What is lost, stated plainly:** a stylesheet cannot make an existing
panel glass. `Glass<Card>` is a code edit, not a rule. That is the price
of the topology being in the type, and it applies to `ScrollView` too.

### 1.2 Why blur only, first

Refraction and specular need the SDF *gradient*; blur does not. Dropping
them for the first cut removes, wholesale: the gradient's `sign(p − c)`
term (which the first draft got wrong, so the left and top rims would
have refracted and lit backwards), its `0/0` degeneracy over the entire
inner rect, the `refraction = 0` division, a `rim` field, and the choice
of a specular exponent. Roughly half the review's maths findings are
about machinery this milestone does not build.

What remains is `backdrop-filter: blur()`, which is the part that is
architecturally interesting and the part every other decision hangs on.

### 1.3 Why the GPU waits

Reviewed concretely: `G+1` render passes (a texture copy cannot happen
inside a render pass), two scratch textures ping-ponged, a second
pipeline, a sixth and seventh bind-group entry, a **linear** sampler
(the existing one is `Nearest` deliberately, so glyphs stay crisp), and a
fix to `draw`'s `Overflow` retry so a second attempt does not inherit the
first's split points. Two to four days, not the half day the first draft
implied.

And the parity gate cannot absorb it as written: `tests/parity.rs` bounds
the *count* of differing pixels at 0.2%, measured at 0.01%. A glass panel
differs over its whole area if the two blurs disagree anywhere — a
400×200 panel is 8.7% of a 720p frame, 43× the bound. Making that gate
meaningful means matching the CPU blur bit-for-bit on the GPU, which
essentially forces a compute shader. That is its own milestone's worth of
work and it is not this one's.

**Cost of waiting, stated:** the two backends visibly diverge for anyone
running `LUMEN_BACKEND=gpu`. That is a first for this project and it is
the reason this is a decision rather than an omission.

## 2. What M18 must deliver first

Glass is blocked on it, and it is worth doing on its own merits.

`Damage` is `None | Rect | All`, and `union` is a **bounding-box merge**.
Correct glass needs damage to cover what the panel *reads*, so `D ∪= src`
— and with a single box that goes badly: a scrollbar thumb moving
(8 000 px) unioned with a glass toolbar's source region (149 000 px)
gives the whole 960 000-px window. Any glass panel plus damage anywhere
else becomes a near-full repaint, and the 13 µs damaged frame
(`docs/results-simd.md`) is gone.

M18 is: **`Damage` carries a small set of regions**, exactly unioned,
with the shell rendering each. It also fixes a defect this repository
already knows about and has not fixed — `docs/damage-amplification-decision.md`
records a `Shrink` container's clip change damaging a whole column,
measured 37× (1 562 px → 57 600 px), for the same reason.

What glass needs from it, specifically:

1. an exact union, so `D ∪ src` does not swallow the frame;
2. an **intersection** predicate with `None` and `All` arms spelled out
   (`All` short-circuits; `None` is empty), because the dependency rule
   is a test against `D`;
3. the set surviving `take_damage_for_age`. The fixpoint cannot live
   only in `Diffing::finish`: `finish`'s result is banked into `pending`
   and re-merged across up to `AGES-1` frames, and **a merge is not
   closed under the dependency rule**. Worked example: two frames of
   damage that each miss a panel can merge into a box that clips it,
   so a panel correct at buffer age 1 is stale at age 3.

M18 should get its own plan doc when it starts. It is listed in
`AGENT.md`'s queue ahead of this one.

### 2.1 And a container that can overlap its children — T11.6

**A glass panel with nothing behind it is a tinted rectangle.** A
`column` allocates non-overlapping rects, so a toolbar laid out above a
scroller has the page behind it and nothing to blur. The motivating
scene does not merely look better with overlap; it does not exist
without it.

`Popup<W>` supplies more of this than it first appears — out of flow,
zero size in the parent's arithmetic, `Anchor::Over`, painted last in
the overlay phase, and measured with `max_w = limits.max_w` so a `FILL`
child does span the parent. What it also supplies is dismissal, and
that disqualifies it: `pass.rs` closes the topmost overlay on Escape and
**all** open overlays on a press outside, *consuming that press*. A
permanent bar built on `Popup` would vanish on the first click elsewhere
in the application and swallow it. That is not a quirk to route around.

`docs/plan-widgets.md` T11.6 built it: `#[widget(stack)]`, children
sharing the container's rect, painted in field order, hit in reverse,
with the reserved `W1004` overlap lint shipped alongside and scoped to
sequential containers. **Landed 2026-09-10**, so this is no longer a
blocker — `Stack { content: ScrollView<Rows>, bar: Glass<Toolbar> }` is
expressible today and only M18 remains.

## 3. The command

```rust
Glass { rect: Rect, radius: f32, blur: f32, tint: Color },
```

Four fields, so `glass(rect, radius, blur, tint)` is `shadow`'s shape
plus a colour, and there is no seven-argument transposition hazard.
When refraction and specular arrive they come as a params struct, in the
shape `Stops` already uses — **not** as four more positional `f32`s.

- `blur` is a Gaussian standard deviation in **logical** px.
- `tint` multiplies over the blurred backdrop; `tint.3` is its strength.
  Alpha 0 is a legitimate value — a pure frosted panel — which is why
  `DisplayList::glass` must drop only on `!has_area(rect)` and **not**
  copy `rounded_rect`'s `color.3 == 0` guard.

**Budget.** `size_of::<Cmd>()` is 60, set by `Gradient`'s 56-byte payload
plus the tag; `Glass` is 24. T19.0 asserts **both** — `Cmd == 60` and the
`Glass` payload against 56 with the measured 24 in the message. Asserting
only the first is a bound 32 bytes from anything it constrains, which is
the shape `docs/plan-invalidation.md` T16.2 and the `.bss` ceiling were.

**Corner radius is *not* absent, and the first draft was wrong to leave
it out** (corrected 2026-09-10 while building T19.0). It reasoned that a
panel's shape could come from `Glass<W>`'s own `Style.radius` and a clip
around it — but clips here are axis-aligned rects, so there is no
rounded clip to lean on and a panel would have had square corners.
`RoundedRect` and `Shadow` both carry their own radius; this carries one
too, and the payload is 28 B against a 56 B budget either way.

## 4. Where it is drawn from

`Surface::render` walks commands **in order into one buffer**, so when a
glass command is reached the backdrop is already the destination. There
is nothing to pass in and no layer to composite. That asymmetry is worth
stating because CSS files `opacity` and `backdrop-filter` under one
heading and the intuition is backwards here: group opacity needs a
subtree rendered *in isolation* and is not in this plan; a backdrop needs
the pixels that are already there.

A **translucent** card needs none of this and already works. Verified
2026-09-10: a translucent rounded rect over content whose colour changed,
rendered by `render_within` against a full render — 18 731 px inked,
**0 px differ**. It is composed *forward*, so damage triggers a replay
and the replay does the compositing. Glass is composed *backward*, off
finished pixels, and that is the entire source of its cost.

### 4.1 The composite

For each pixel in the panel:

1. **Coverage** comes from `fill_rounded_within`'s *structure*, not from
   a fresh SDF: hard spans in the straight regions, the ramp only in the
   corner bands. The first draft claimed `clamp(0.5 − d, 0, 1)` was "the
   identical ramp" — it is identical only inside the corner bands, and
   elsewhere that function fills whole pixels hard, floor/ceil-snapped.
   On a rect with fractional coordinates the two disagree by up to
   254/255 along a straight edge. Sharing the structure is what makes
   "pixel-identical to a `RoundedRect` of the same geometry" achievable
   at all, and it keeps glass looking like every other panel.
2. **Tint:** `c = mix(backdrop, tint.rgb, tint.a)`.
3. **Blend** `c` into the destination at that coverage.

### 4.2 The blur

Three box passes with running sums — O(1) per pixel in the radius **on
the CPU** — at the size SVG's `feGaussianBlur` specifies:

> `d = floor(s * 3 * sqrt(2*pi)/4 + 0.5)`; if `d` is odd, three boxes of
> size `d` centred on the output pixel; **if `d` is even, two boxes of
> size `d` centred on opposite pixel boundaries and one of size `d+1`
> centred on the pixel.**
> — <https://www.w3.org/TR/SVG11/filters.html#feGaussianBlurElement>

The even case is not a detail to skip: an even box has no centre pixel,
so three equal even boxes shift the image half a pixel per pass, 1.5 px
total. And the even case is the *common* one here — the downsample below
targets `σ' ≈ 2`, where `d = 4`.

Five things the first draft left unspecified, each of which changes
pixels:

- **Downsample factor.** `k = clamp(⌈σ/2⌉, 1, 8)` was wrong twice: it
  drops the per-pass sigma below the spec's own `s ≥ 2.0` precondition
  for every σ that is not an even integer, and it delivers a blur up to
  **24% away** from the requested σ, in plateaus (σ = 4.0, 4.5 and 5.0
  all render identically, then σ = 6 jumps 50%). Instead **search
  `k ∈ 1..=8` for the `k` minimising `|k·σ_box(d(σ/k)) − σ|`** — eight
  iterations of `floor` and `sqrt` per command, not per pixel, bounding
  the error at 10%. The results doc prints *delivered* σ beside
  *requested*, so the quantisation is visible rather than re-derived.
- **Capture dilation.** `3σ` is exactly the three-box reach with zero
  margin at every even-`d` operating point, and it counts none of the
  downsample's own `k`-px footprint, the bilinear upsample's, or the
  sample's own ±1 tap. Use `ceil(3σ + 2k + 2)`, snapped outward to a
  multiple of `k`. **The dependency in §5 must use the same
  expression** — a capture wider than the dependency is stale in its
  outermost ring, and the oracle would pass on it.
- **Downsample grid anchor.** Anchored to the **surface** origin
  (`floor(x/k)`), never the capture's. Anchored to the capture, a panel
  at a fractional or scrolled position samples a different phase each
  frame and the blur shimmers — and both the "stable frame to frame"
  check and the incremental-vs-rebuild oracle are blind to it, because
  the capture rect is identical in the two renders being compared.
- **Border rule.** Samples outside the captured region clamp to its
  edge, and the running sum is normalised by the number of contributing
  samples, so a partial box does not darken. This also answers the
  window-edge case, and matches what a GPU sampler does by default when
  that backend arrives.
- **Intermediates are `u16`, not `u8`.** Three passes round three times
  at ≤0.5 LSB each and band visibly on a smooth backdrop. 2× scratch,
  one rounding at the end.

### 4.3 Colour, stated once

The blur is computed on **straight, non-premultiplied 8-bit sRGB**, and
that is correct *here and only here*: `Surface::blend` and `blend_px`
both write `α = 255` unconditionally and `CLEAR` is opaque, so the
destination alpha is a constant and straight equals premultiplied. **A
transparent-window path would invalidate this** and must premultiply
before the box passes. T19.2 puts that sentence next to the loop.

It also means the result matches **CSS `backdrop-filter` on HTML
content**, not SVG's `feGaussianBlur` — whose
`color-interpolation-filters` defaults to `linearRGB`, so a browser's
SVG blur works in linear light and this does not. Using the SVG spec's
box sizes buys the right *kernel*, not the right *colour space*, and the
first draft claimed the latter.

## 5. Damage: the dependency

`Recording::glass` reports two things, not one:

- `observe(hash, rect)` — as every command does. Skipping it
  desynchronises `Diffing`'s command count against its slots and makes
  `finish` return `Damage::All` for the rest of the frame; and the
  panel's own `blur`/`tint` change would never be reported. The hash
  covers **every field by bit pattern**, `blur` included, since `blur`
  sizes the dilation.
- `observe_dep(src)` — the region the composite reads, `rect` dilated by
  §4.2's expression.

Then, wherever the frame's damage is finalised:

```
for each dep src, while D changed and at most G+1 rounds:
    if D ∩ src is non-empty then D ∪= src
```

Four corrections to the first draft, all from the review:

1. **`D ∪= src`, not `D ∪= dst`.** The annulus `src \ dst` is what the
   panel reads and does not write. Left un-damaged it holds the previous
   frame's *final* pixels — including commands painted **after** the
   glass, and including **the panel's own previous output**, which
   re-blurs and smears a little more every frame. Both reviewers who
   looked at this found it independently, from different code.
2. Since `src ⊇ dst` by construction, the rule has one clause, not two.
3. **It also runs on the banked union**, not only in `finish` — see §2.3.
4. **No round cap, and no fallback.** `D` only grows, the rects are fixed for
   the frame, and re-adding a contained rect is a no-op, so the fixpoint
   terminates in at most `G+1` rounds *provably*. The first draft's cap
   and its `Damage::All` fallback were an untriggerable branch, and per
   `AGENT.md` a branch that cannot fire cannot be shown to fail. The
   first draft specified a cap and a `Damage::All` fallback for it;
   assert the bound instead.

`observe_dep` gets a **provided body** so `Rebuild`, `Streaming<N>` and
any host strategy are unaffected until they opt in — the shape
`docs/damage-amplification-decision.md` already pre-registered for this
trait. `Retention` is a type parameter and never `dyn`, so object safety
is not a constraint. `Streaming<N>` needs a fixed dep array whose
overflow sets `Damage::All`, mirroring how its slot overflow already
behaves.

**Which coordinate space.** `Recording` wraps `Scaled`, so `observe` and
`observe_dep` see **logical** rects while `render_within` consumes
physical ones. At scale 1.0 — every test, every desktop default — this is
invisible. T19.2 states the space in the doc comment and
`tests/scale.rs` gets the row.

## 6. The backdrop cache

The optimisation that makes glass cheap, and the reason §2's rect set is
enough rather than merely necessary.

One buffer per live glass panel, holding its blurred backdrop. On a
frame:

- nothing intersects the panel's `src` → reuse it; cost is the composite
  alone;
- something does → re-render commands `0..i` clipped to `src` into the
  cache, re-blur, composite.

**The win is that a recompute is bounded by the panel's own area rather
than by a union across the frame.** The scrollbar-plus-toolbar case stops
costing a full-window replay and starts costing one toolbar-sized prefix
replay.

It is a layer cache scoped to glass panels — bounded, countable, and
nothing like a general layer tree. Cost: one buffer per panel (a
1200×124 toolbar is ~600 KB) and a second validity question. T19.4 lands
it *after* a correct uncached implementation and is measured against it,
so the cache's value is a number and not an assumption.

## 7. Threading

`Surface`'s whole safety argument is that bands own disjoint rows. A blur
reads rows another worker owns.

**The rule:** the command list is segmented at glass boundaries; each
segment is banded as today; between segments the workers join and the
capture and blur run serially. **The composite rides the head of the
*next* segment** — each band composites the glass over the rows it owns,
culled exactly as every other command is — which keeps the count at
`G+1` scopes and `G+1` joins. Giving the composite its own scope would
make it `2G+1`, which is what the first draft's text described while its
arithmetic said otherwise.

Four things it must also say:

- **The clip stack must be reconstructed at each boundary.**
  `Surface::render` builds its stack by walking from index 0, seeded with
  the band ∩ damage rect. A band starting mid-list would begin at depth 1
  and not know it is inside a scroller — and a glass toolbar inside a
  scrolling view is the motivating scene. The segmenter computes the
  clip prefix serially, once, and each band starts with
  `[band ∩ damage] ++ prefix`. `PopClip`'s floor stays at index 0, not at
  the prefix depth, since a `PushClip` before the glass and its matching
  `PopClip` after it is the normal case.
- **`Surface::render` takes a command range and a clip prefix.** It
  cannot take `&[Cmd]` — the text arm needs `list.str_of` — so the
  `&DisplayList` stays and a `Range<usize>` joins it. That is a change to
  the one rasteriser entry point all three callers use, and it *is* the
  task, not a precondition of it.
- **The borrow structure permits it.** N sequential `thread::scope`s over
  the same `&mut [u8]` are fine; only the hoisted `let slice = &mut
  self.pixels[..]` has to move inside the loop so its borrow ends each
  iteration. Sub-row writes need no `unsafe`: disjointness comes from the
  row split, and a worker sub-slicing a row it already owns exclusively
  is an index range, not aliasing.
- **The worker count is recomputed per segment.** `workers_for` is
  called once today, on the whole damaged pixel count. Spawning 32
  workers for a trailing segment of three commands is pure loss.

**One implementation, not two.** The composite is a `Surface` method,
called by the whole-surface walk and by the segmented bands alike.
`render_banded` can decline (small frame, `images.is_some()`,
`workers < 2`), and the caller then renders through `Canvas::whole()` as
one band — so a second copy for the threaded path is exactly the fork
`Surface`'s doc comment records rejecting.

**Unmeasured, and the plan says so:** the per-spawn cost this `G+1`
arithmetic matters against does not exist as a measurement anywhere in
this repository. The "5–10 µs to wake" figure in `software.rs` and
`docs/results-threading.md` is an unsourced assertion. T19.5 measures it
or drops the claim.

## 8. Tasks

| id | task | done when |
|---|---|---|
| **T19.0** | `Cmd::Glass`, `DisplayList::glass`, `Painter::glass` with a degraded default, explicit overrides on **all four** wrappers, arms in all three renderers, `hash_into`, `Scaled`, both size assertions. | A glass command survives every wrapper unchanged, and renders as a tinted rounded rect on both backends. |
| **T19.1** — **DONE 2026-09-10** | `Glass<W>`: the `NodeDesc` splice, `layout`/`place`/`paint`/`hit`, `blur` and `tint` as fields. | A `#[widget(stack)]` of `ScrollView<Rows>` and `Glass<Bar>` overlaps by 19 200 px and emits the panel *after* what it covers — which is also what makes T19.2's oracle constructible. |
| **T19.2** — **DONE 2026-09-10** | Software capture, three-box blur, composite. Single-threaded. `observe_dep` and the fixpoint on M18's region set. The damage oracle — whose scene needs T11.6's `stack`, since a panel with nothing behind it cannot go stale. | Byte-identical over a moving backdrop across 8 steps (7 partial, 7 with the panel's pixels moving), and a still frame with a panel damages nothing. |
| **T19.3** | Blur quality: the `k` search, the odd/even rule, `u16` intermediates, grid anchor, border rule. A results doc with the σ × panel-size sweep and delivered-vs-requested σ. | The sweep exists; delivered σ is within 10% of requested at every point in it. |
| **T19.4** | The backdrop cache, measured against T19.2. | A frame whose damage misses every panel costs the composite only, and the doc says by how much. |
| **T19.5** | Threading: segmentation, clip prefix, per-segment worker count, composite in the next segment's head. | `just threads` byte-identity holds with glass in the scene, at every swept strip height, **with the split asserted** via `Canvas::bands_for`. |
| **T19.6** | Embedded: confirm the variant costs the firmware nothing. | `just firmware-size` and `scripts/check_text.sh` restated in `docs/results-embedded.md`. |

**Deferred, deliberately, each with its condition:** refraction and
specular (when blur has shipped and someone wants the material rather
than the effect); the GPU backend (§1.3); group opacity (needs a real
layer, §4).

**Sizing.** T19.0 and T19.1 are a day together. T19.2 is the milestone's
centre and should be budgeted as such. T19.3 is a day including the
sweep. T19.4 and T19.5 are a day each. T19.6 is an hour.

## 9. What must be shown to fail

| test | injection | proves |
|---|---|---|
| T19.0 wrappers | drop the `glass` override from `Deferring` | a wrapper's provided body rewrites the command — the defect this review already found live and fixed in `1de77c6` |
| T19.0 budget | add `[f32; 8]` to `Glass` | the payload assertion binds, not just the `Cmd == 60` one |
| T19.2 oracle | delete `observe_dep` | the panel goes stale over a moving backdrop |
| T19.2 still frame | make the dependency unconditional | a still frame with glass stops being free |
| T19.2 annulus | dilate the dependency to `rect` instead of `src` | the outer ring reads last frame's final pixels |
| T19.3 blur | use three equal boxes when `d` is even | the 1.5 px shift the spec's odd/even rule exists to prevent |
| T19.3 capture | shrink the dilation by `2k` | the rim samples clamped edges; assert the differing-pixel count and print it |
| T19.4 cache | never invalidate the cache | the panel freezes over a moving backdrop |
| T19.5 threading | composite from a source clamped to the band's own rows | a panel spanning bands blurs wrong |

## 10. The three vacuous tests this will otherwise produce

All three are `AGENT.md`'s documented shapes, specific to this feature,
written down before the tests rather than after someone finds them.

**A static backdrop cannot exhibit a stale panel.** If T19.2's scene does
not move behind the glass, incremental and rebuild agree about a frame in
which nothing happened — T16.7's invalidation oracle, three milestones
later. The backdrop must move **and the test must assert that it moved**,
not only that the two renders match.

**A backdrop that moves by changing the command count degrades to
`Damage::All`.** The obvious moving backdrop is a scrolling list — but
T16.12's paint-side culling emits only the rows a viewport shows, so
scrolling changes the command count, and `Diffing::finish` answers a
length change with `Damage::All`. The incremental arm then takes the
full-redraw path and the oracle compares two identical full renders, with
`observe_dep` deleted and still green. **The backdrop must move without
changing the command count** — a label whose text changes, or content
translating inside a fixed-length list — and the test must assert
`Damage` is a region set on the steps it counts, printing the count
beside the bound.

**An opaque tint hides everything this feature does.** At `tint.3 = 255`
the blur and the backdrop are both invisible, every injection above still
produces correct pixels, and the suite is a rounded-rect test in costume.
This is the T15.12 GPU damage gate's failure exactly — a half-cleared
boundary pixel under *opaque* content is overwritten and the artefact
never survives to be compared. **Every glass test uses a translucent tint
over content with structure in it**, and asserts the backdrop is visible
before asserting anything else.

The same trap sits inside T19.0's own done-when. A realistic tint is
something like `Color(255,255,255,40)`, and the degraded fallback paints
that as a 16%-opacity rectangle — nearly nothing. So the fallback raises
alpha to a documented floor, and T19.0 asserts a **pixel count**, not
"renders as a tinted rounded rect".

## 11. What this does not buy

- **No group opacity.** §4.
- **No blur of a node's own content.** `Cmd::Glass` blurs what is behind
  a rect. `filter: blur` on a node blurs the node, which is a layer.
- **No rotation, `clip-path` or SVG.** `docs/results-vello.md` §4 is
  still the map of what is on the far side of the line.
- **No overlap of its own.** `Glass<W>` blurs what precedes it in paint
  order; it does not create the overlap. That is `plan-widgets.md`
  T11.6's `stack` axis, and §2.1 says why a panel is useless without
  it.
- **No measured cost.** Nothing in this document is a measurement except
  `size_of::<Cmd>() == 60`, `Style`'s 72, and the translucency check in
  §4. T19.3 produces the sweep; until then "glass is cheap" is an
  argument with no evidence.
