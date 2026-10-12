# Performance

The framework's own position on this is in [What it
costs](./costs.md): the tree walk is not where frame time goes. This
chapter is about how to find out where it *does* go, and how to add a
measurement that can say no.

## The budgets

Two, and they are judged automatically:

| budget | what it bounds |
|---|---|
| 16 ms | one frame |
| 100 ms | click to presented |

`scripts/ui_check.py latency` measures both against a live window and
fails when either is exceeded. It is the list of live checks — do not
restate it elsewhere, because it drifted to a stale copy once already.

## Profiling a frame

```text
just profile            # per-stage cost, headless, median of 50
just profile-simd       # with the rasteriser's SIMD path
just profile-threads    # with band-parallel rasterisation too
```

`FULGURITE_PROFILE=1` is what turns on the per-stage report. `app.perf` over
the agent protocol reports the same stages from a live window, plus
input-to-presented latency over recent frames.

The numbers belong in `docs/results-frame-cost.md`, not in a comment and
not in this chapter.

## The report that started all of it

Worth reading `docs/results-frame-cost.md` in full once, because the
shape of the investigation is instructive. The complaint was "clicking a
button takes almost a second". Layout and paint were a microsecond.
Nothing the agent could see explained it — because `app.perf` reported
only layout and paint, which were the two stages that were fine.

Everything was in rasterisation and in a dev-profile setting. The fix
included making `app.perf` report *every* stage, so the next such report
would be answerable.

**A profiler that only reports the stages you suspected is not a
profiler.**

## Adding a measurement that can say no

The house style, from `AGENT.md`:

> Decisions backed by measurement get their own `docs/results-*.md`: the
> number, the threshold it was judged against, and the verdict — so the
> gate can say no and the next reader can see why it did.

Four things make the difference between a gate and a decoration:

**Print the measured quantity next to the bound.** Three of the six
vacuous gates found on this repository were caught this way and by
nothing else. A 6% bound over a 0.01% measurement and a 2 KB ceiling over
a measured zero both read as reasonable until the two numbers sit side by
side.

**Bound the quantity you meant.** A damage check once bounded a row's
height with the *list's* total height — a limit two orders of magnitude
above anything the assertion could see.

**Break the thing and watch it go red.** Not change a value: delete the
guard, invert the condition, remove the clip. Then restore, and watch it
go green. Put the injection and its effect in the commit body, because
that number is the only evidence the gate works and nobody can re-derive
it from a passing test later.

**Beware the minimum.** A benchmark arm reporting a minimum over its
iterations selects the cheapest mode of a bimodal distribution. On the
damage arm that meant the published figure described the frames where a
row's width happened not to change — see [Damage and
invalidation](./damage.md).

## What performance work here has actually looked like

Three of the biggest wins were not algorithmic:

* A dev-profile setting, because debug assertions put bounds checks
  inside every inner loop of the rasteriser.
* A clip that was not being pushed, so every band re-shaped every text
  run — see [The rasteriser](./rasteriser.md).
* Measuring text properly instead of estimating it, which fixed both a
  layout bug and a measurement.

None of them would have been found by making the tree walk faster.
