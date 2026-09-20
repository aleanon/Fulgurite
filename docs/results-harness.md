# Results: M12 — the test harness

*M12, `docs/plan-harness.md`. Owns every figure that plan cites.*

## T12.1 — a locator and expectation API

**Landed 2026-09-20.** `lumen_static_agent::harness::TestApp` drives
anything that implements `AgentHost` — every host in this repository,
headless or windowed — through the selector grammar and
`Agent::dispatch_input` that already existed.

### The gate: the same session, both ways

`examples/counter-window --drive` is the hand-written agent session;
`--drive2` is that session against the harness. **Both are kept and both
run**, because the comparison is the evidence — a harness that were only
shorter would be a convenience, and the claim is that it says more.

```
drive:  7 clicks by author id through ui.getLayout — all assertions passed
drive2: 21 harness steps, all assertions passed
```

The hand-written version needs a `centre` closure that assembles
`ui.getLayout` JSON and unwraps four array indices, plus a `click`
closure over it. The harness version has neither.

### What it says that the hand-written one does not

Each of the three is an injection, run for real:

**1. A missed selector teaches.** `#inc10` misspelled as `#inc100`:

```
drive2: input.click {"selector":"#inc100"}:
  {"code":-32001,
   "data":{"available_ids":["card","caption","value","pill","steps",
                            "dec10","dec","inc","inc10","reset"]},
   "message":"selector `#inc100` matched nothing"}
  after: click #inc -> click #inc
```

The protocol already answered a miss with the nearest candidates; the
harness surfaces that text rather than flattening it to "assertion
failed". The id that was meant is in the list, and so are the two steps
that preceded the failure.

**2. A click that reaches no handler fails at the click.** `#caption` is
a label:

```
drive2: click #caption: the input reached no handler (`handled` was false)
  after: click #inc -> click #inc -> click #inc10 -> expect_text #value == "12"
         -> expect_text #pill == "POSITIVE" -> click #dec10 -> click #dec10
         -> click #dec -> expect_text #pill == "NEGATIVE"
```

Without the `handled` check this would surface three assertions later as
an unchanged count — which is the M16/T16.7 shape `AGENT.md` records,
where every click landed outside its target and the oracle compared two
identical no-ops.

**3. The locator's accepting direction.** `expect_absent("#reset")`:

```
drive2: expect_absent #reset: it resolved to
  {"bounds":[184.0,347.5,256.0,385.5],"kind":"button","label":"Reset",...}
```

Without this, a resolver that silently matched the root would pass every
`expect_text` in the session against the wrong node.

### No auto-wait, and the API says so

Every method is synchronous: it pumps, and by the time it returns the
frame is settled. There is no animation scheduler and `ui.waitSettled`
is trivially true, so a retry loop would be machinery pretending to
solve a problem this architecture does not have. If a scheduler ever
arrives, waiting goes in with it.

## T12.2 — golden images with a diff artifact

**Landed 2026-09-20.** The *comparison* already existed —
`tests/damage_oracle.rs` compares whole pixel buffers. What it did on a
mismatch was `assert_eq!` two 264 KB `Vec<u8>`s: no count of differing
pixels, let alone a picture.

### The gate: change a radius by 1 px

```
  ok  unchanged frame: matches
  ok  1 px radius change: 92 of 9600 pixels differ, inside [20, 16, 100, 64]
  ok  diff image: 92 red pixels marking the change (of 92 differing)
```

The count is a number. **This is the diagnosis** — the diff image, over
a 120×80 frame holding one rounded rect at `[20, 16, 100, 64]`:

```
  ............................................................
  ...........RRRRR............................RRRRR...........
  ..........RR....................................RR..........
  ..........R......................................R..........
  ............................................................
                          (middle rows clear)
  ............................................................
  ..........R......................................R..........
  ..........RR....................................RR..........
  ...........RRRRR............................RRRRR...........
  ............................................................
```

The four corners, and nothing else — which is the only place a 1 px
radius change can move a pixel. "1,482 pixels differ" could never have
said that.

The test asserts it three ways rather than eyeballing: the count is
under a tenth of the canvas, the differing region is **inside** the
rect's own bounds, and the region is not the whole canvas. And the diff
image marks *exactly* the differing pixels — 92 red for 92 differences,
an equality rather than a `> 0`.

### A first run records and does **not** pass

An unreviewed reference must not be indistinguishable from a checked
one. `Verdict::Recorded` is not a match, and the message says to review
it and re-run.

### Two rules enforced rather than written down

**`truetype` is refused.** `FontStack::system` takes the first of five
candidate faces that exists on the machine and silently serves both
weights from the regular face when no bold is found, so a golden
recorded under that feature records the recording machine's font
install. `golden::compare` panics with the feature on, and the test file
is `#![cfg(not(feature = "truetype"))]` — so `cargo test --features
truetype` reports `0 passed` rather than quietly comparing.

**PPM, not PNG.** `Canvas::to_ppm` already writes it and it needs no
dependency. The PNG encoder lives in `lumen-static-agent`, which depends
on `lumen-static` with `features = ["observe"]` — not optional — and
Cargo unifies features across a selection, so a dev-dependency on it
from `lumen-static-shell/tests` would switch `observe` on for
`alloc_gate.rs`'s process and grow `NodeDesc` from 48 B to 72 B. Same
unification hazard M13's feature polarity is argued from.

### One test whose bound was computed from the wrong quantity

The red-pixel assertion first read the diff's body at a hardcoded offset
of 15; the header is 14 bytes, so every RGB triple was read one byte out
of phase and the count came back **0 red for 92 differences**. It found
itself immediately because the number printed beside the bound — in a
test written to catch exactly that shape.
