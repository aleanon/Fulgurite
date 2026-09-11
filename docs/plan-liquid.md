# M22 — the complete liquid material

> **Status: planned 2026-09-11, not started.** Five tasks, and they are
> deliberately in this order: the first four keep the material
> panel-sized and cheap, and the fifth is the only one that changes what
> it can be *used* for.
>
> Builds on M19 (`docs/plan-glass.md`) and M21's bevel
> (`docs/results-glass.md` §5b). What exists: backdrop blur, edge
> refraction over a spherical bevel, one specular lobe, a tint.

*Each task is one commit; each ends with `just test` green. Task ids go
in commit subjects. Every task names the injection its tests must go red
under, and §7 names the vacuous test each one will otherwise produce.*

---

## 1. The budget, first, because it decides two of the five

Measured 2026-09-11:

| | bytes |
|---|---:|
| `Material` | 24 |
| `Cmd::Glass` payload (`rect` 16 + `radius` 4 + `Material`) | 44 |
| `Gradient`'s payload — **what sets `Cmd`'s size** | 56 |
| `Cmd` | 60 |

**Headroom is 12 bytes: three more `f32`s, and the fourth grows every
command in every list.** That is not a style preference, it is the
constraint T22.0–T22.3 have to be designed inside, and it is why the
morph below is the task with a real decision in it rather than a real
formula.

Spend: T22.0 takes two (`dispersion`, `saturation`), T22.1 takes one
(`adapt`), T22.2 takes none. T22.3 needs at least twelve and is
therefore **over budget by construction** — §5.

## 2. T22.0 — dispersion, inner shadow, saturation

The biggest visual return per hour, and all three are arithmetic in the
loop that already runs.

**Chromatic dispersion.** Glass splits light by wavelength, and the
fringe at a thick edge is most of what separates "a lens" from "a blur
with a highlight". Sample the three channels at slightly different
displacements:

```
k = refraction · u²
r at p + n·k·(1 + dispersion)
g at p + n·k
b at p + n·k·(1 − dispersion)
```

Three bilinear samples instead of one, **in the rim band only** — `u` is
0 in the interior, so the interior is untouched and costs nothing.

**Inner shadow.** The specular already computes `dot(n, light)` and
discards the negative half. Darken there instead of discarding. A rim
lit on one side and shaded on the other is what reads as thickness, and
it is free: the dot product is already in hand.

**Saturation.** Multiply the blurred backdrop's chroma before the tint
mixes in. Frosted glass that desaturates looks like fog; the reason
colours read *through* a real panel is that the material does not eat
them. One lerp against the pixel's own luminance.

## 3. T22.1 — adaptive tint, the one that makes it usable

A fixed tint is legible over the backdrop it was chosen for. Apple's
material goes light over dark content and dark over light, so whatever
is *on* the glass stays readable as the content scrolls under it. That
is the difference between a material and a decoration.

**The input is already computed.** The blur downsamples the captured
source by `k`; the mean luminance under the panel is a reduction over
that small buffer, not over the full-resolution capture. A box blur
preserves the mean, so it can be read after the passes.

Three things to get right, each of which is a way to get it wrong:

- **Reduce over the panel, not over the capture.** The captured region
  is dilated by `3σ + refraction + 4`. A panel near a window edge whose
  mean came from the dilated source would adapt to content it is not
  over.
- **Smooth, not stepped.** A hard threshold flickers when the mean sits
  on it and the content scrolls. Interpolate.
- **And that may not be enough.** If it still oscillates, the fix is
  temporal damping, which is *state* — and a command has none. It would
  live on `Glass<W>` as an `Animated<f32>`, which is the same place
  T22.2's press animation lives. **Do not add it pre-emptively**:
  measure the oscillation first, and if smoothing alone settles it, the
  state is not needed.

## 4. T22.2 — press deformation

`NodeState::pressed` already exists and `Style::pressed_dim` already
uses it. A glass button should *squash*: `rim` and `refraction` rise
briefly under the finger, so the edge thickens and the lensing deepens.

**No command change and no renderer change.** `Glass<W>` reads the
pressed flag and modulates the `Material` it assembles in `paint`.

One shape question the task has to answer: `Node::paint` takes `&self`,
so it cannot advance an animation. The value has to be stepped where
state is already stepped — `Host::tick` — which means an `Animated<f32>`
on the widget and a decision about which hook advances it. That is the
whole of the task's difficulty; the maths is a multiply.

## 5. T22.3 — the morph, and the decision it forces

The "liquid" in the name is panels **stretching and merging like
droplets**. It looks like it needs arbitrary shapes — a Bézier pipeline,
which `docs/results-vello.md` §4 puts firmly outside this renderer.

**It does not.** Two merging blobs is a *smooth minimum* of two signed
distance fields: `min(a, b)` softened by a polynomial in `|a − b|`.
Closed form, a few operations, and it produces exactly the surface
tension necking that two pills show as they meet. The composite is
already driven by an SDF, so this fits the architecture **better** than
it fits a path renderer, where the outline would have to be solved for.

Three consequences, and the first is the reason this task is listed
third rather than first:

- **It does not fit the budget.** A second rect is 16 bytes against 12
  of headroom (§1), and by T22.3 the headroom is 0. The options, and
  the task's first job is to choose between them with a measurement:
  **(a)** encode the second blob as an offset and a scale — three
  `f32`s, which fits exactly and covers a pill splitting or an icon
  growing into a panel, but not two panels of different aspect merging;
  **(b)** give the pair its own `Cmd` variant and let `Cmd` grow, which
  costs every command in every list and must be re-budgeted in
  `docs/results-embedded.md`, not asserted away.
- **Coverage stops being `fill_rounded_within`'s.** A morphing outline
  has no straight spans, so the hard-span/corner-ramp structure M19
  kept for silhouette parity no longer applies and every pixel needs a
  distance. That is a real cost increase and it should be measured, not
  assumed small.
- **The normal is a blend.** The bevel needs the gradient of the
  *combined* field, which is the two gradients weighted by which one
  dominates. Getting that wrong shows as a crease where the blobs meet
  rather than a neck.

## 6. T22.4 — the GPU backend, and why it is last

**This is the only task that changes what the material can be used for**,
and everything above is why it is not first.

Measured, `docs/results-glass.md` §2: **~21 ns/px.**

| | cost |
|---|---:|
| a toolbar, 460×64 | 0.6 ms |
| 1080p, full screen | **43 ms** |
| 4K, full screen | **174 ms** |

A panel is affordable and a screen is not. Apple's material is used over
large regions at 120 Hz, which is a fragment shader; ours is a software
rasteriser and single-threaded besides, because T19.5 declined banding —
a bevel reads past its own rows.

The port itself is already specified: `docs/plan-glass.md` §1.3 has the
concrete list (G+1 render passes because a texture copy cannot happen
inside one, two ping-ponged scratch textures, a second pipeline, a
linear sampler beside the deliberately-`Nearest` one, and the
`Overflow` retry fix), and §1.3 also has the parity problem — the gate
bounds the *count* of differing pixels at 0.2%, and a panel differs over
its whole area if the two blurs disagree anywhere.

**Last, deliberately.** Every task above changes the material, and the
shader has to match the CPU pixel for pixel or the parity gate is
meaningless. Porting before the material settles is porting twice.

## 7. Tasks

| id | task | done when |
|---|---|---|
| **T22.0** | Dispersion, inner shadow, saturation. Two new `Material` fields. | A coloured edge under the rim shows a visible fringe; a lit rim has a shaded opposite; a saturated backdrop stays saturated through the panel. `Cmd` is still 60 B. |
| **T22.1** | Adaptive tint from the mean luminance under the panel. One new field. | Sweeping the backdrop from black to white moves the tint monotonically, and a scroll across the midpoint does not flicker. |
| **T22.2** | Press deformation, driven by `NodeState::pressed`. No command change. | Pressing a glass button thickens its rim and settles back, and the panel is unchanged when nothing is pressed. |
| **T22.3** | The smooth-minimum morph. **Decide (a) or (b) of §5 first, with the measurement.** | Two panels at a separation of one rim width show a neck, not two outlines; and the budget decision is recorded with its cost. |
| **T22.4** | The GPU backend (`docs/plan-glass.md` §1.3). | `just gpu-parity` holds with a glass panel in the scene, under a bound stated with the measurement beside it. |

**Sizing.** T22.0 is a day. T22.1 is a day plus however long the
flicker takes to settle. T22.2 is an afternoon once the animation hook
is chosen. T22.3 is two days and one of them is the budget decision.
T22.4 is 2–4 days and is its own milestone in all but name.

## 8. What must be shown to fail

| test | injection | proves |
|---|---|---|
| dispersion | sample all three channels at the same displacement | the fringe is dispersion and not an artefact of the blur |
| inner shadow | discard the negative dot again | the shaded side is computed, not inherited from the backdrop |
| saturation | pass the backdrop through unchanged | the chroma boost is doing something |
| adaptive tint | freeze the mean at 0.5 | the tint is reading the backdrop, not a constant |
| press | ignore `NodeState::pressed` | the deformation is driven by input |
| morph | plain `min` instead of the smooth one | the neck is the smooth minimum and not overlap |
| GPU parity | skip the shader's dispersion | the two backends agree about the material, not just about its silhouette |

## 9. The vacuous tests this will otherwise produce

Each is specific to a task above, and each is a case where the feature
is invisible and every injection still passes.

- **Dispersion over a grey backdrop splits nothing.** Three channels
  sampled at three offsets of the same grey give the same grey. The test
  needs a *coloured* edge under the rim, and must assert the three
  channels actually differ there.
- **Saturation over a grey backdrop does nothing either**, for the same
  reason and with the same fix.
- **Adaptive tint over a uniform mid backdrop never crosses anything.**
  The test has to sweep the backdrop's luminance across the range and
  assert the tint moved monotonically — a single frame proves nothing at
  all.
- **Press deformation with `pressed` never set** compares a panel with
  itself. Assert the flag changed, not only that the pixels match.
- **A morph at either extreme is not a morph.** Fully separated is two
  panels; fully overlapped is one. The property is the *neck*, so the
  test must sit at a separation where a neck exists — and assert that
  the pixels between the two blobs are inked, which is the thing plain
  `min` would not do.

## 10. What this does not buy

- **Not Apple's, even complete.** No environment reflection from a real
  cubemap, no light that responds to device motion (though `light` is a
  field and an application can already animate it), and no system-level
  "Reduce Transparency" fallback — that is a host policy, not a renderer
  feature, and it belongs wherever accessibility settings land.
- **Not full-screen, until T22.4.** §6.
- **No measured cost yet.** Nothing in this document is a measurement
  except §1's byte counts and §6's 21 ns/px, both of which are already
  in `docs/results-embedded.md` and `docs/results-glass.md`. Each task
  adds its own row to the latter.
