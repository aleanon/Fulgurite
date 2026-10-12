# Results: M12 — the test harness

*M12, `docs/plan-harness.md`. Owns every figure that plan cites.*

## T12.1 — a locator and expectation API

**Landed 2026-09-20.** `fulgurite_agent::harness::TestApp` drives
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
layout is current. **The rest of this paragraph was false when written**
(M48/T48.2): it said there was no animation scheduler and that
`ui.waitSettled` was trivially true — but `App::tick`, timers and the
task inbox all outlive a pump, and `ui.waitSettled` answering `true`
through all of them was R13-F1. It now answers from the host (the
animation, the inbox, jobs in flight, the next timer) or `null` from a
host that cannot say. The harness still does not poll for its caller.

## T12.3, amended — a replay that diverges is an error (M48/T48.10)

T12.3 is recorded in `docs/plan-harness.md`; its gate could not fail.
`crates/fulgurite-agent/tests/session_replay_fails.rs` inverts the
review's probes (R13-F6), and each inversion was red at M48/T48.0:

| case | at T48.0 | now |
|---|---|---|
| corrupted recording (steps 2 and 3 edited) | `Ok(())`, fresh n=1 / recorded 3 | `Diverged { step: 2, .. }` (error -32001, recorded `handled: true`) |
| recorded sheet `dark-compact`, host `default` | `Ok(())`, fresh n=1 | `Mismatch { field: "stylesheet" }`, fresh n=0 |
| export with `#does-not-exist` as step 3 | 2 discarded replies, a placeholder, passes | panics: "step 3 (`input.click`) failed: …" |

**The final-tree digest alone would not have caught the third**: a click
on a selector that matches nothing changes nothing, so the tree at the
end is the recorded one. With the per-step assertion removed from the
exporter, that export passed. Both assertions are in every export, and
the two included exports are compared byte for byte with what the
exporter writes (`FULGURITE_BLESS=1` regenerates them).

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

**Corrected 2026-09-25 (M43/T43.9).** The sentence above was true of
`Verdict` and false of both tests that used it: `golden.rs` re-ran the
comparison after a recording and passed, and `failure_screen.rs` printed
every verdict but `Differs` and passed — so a deleted golden was
re-recorded green (M38's I-NEW-4). Both now fail on `Recorded` with
"a first run is not a pass".

### Where the gate runs

**Nowhere, until M43/T43.9** (M38's R17-F1 and R9-F4). Both files need
`truetype` off (below), and every `--workspace` build turns it on, so
`golden.rs` compiled to zero tests and `failure_screen.rs` lost its
golden test in every automated run. `just dev-shape` now runs
`cargo test -p fulgurite-shell --test golden --test failure_screen`
with the shell built alone, and ends with `scripts/check_test_counts.sh`,
which fails on any integration-test binary listing zero tests unless
`scripts/zero-tests.allow` says why — `golden` is allowed there only
while a justfile line runs it alone, which the check verifies.

### Two rules enforced rather than written down

**`truetype` is refused.** `FontStack::system` takes the first of five
candidate faces that exists on the machine and silently serves both
weights from the regular face when no bold is found, so a golden
recorded under that feature records the recording machine's font
install. `golden::compare` panics with the feature on, and the test file
is `#![cfg(not(feature = "truetype"))]` — so `cargo test --features
truetype` reports `0 passed` rather than quietly comparing. (A `0 passed`
is itself refused now, above.)

**Text tests shape a committed face (M43/T43.11).** The same hazard one
level down: the shaping, glyph-placement, complex-text and baked-face
tests read the machine's fonts and *returned early, passing,* when none
was installed (M38's R17-F7). They now load
`crates/fulgurite-shell/tests/fonts/DejaVuSans.ttf` through
`FontStack::from_bytes` and print `face: DejaVuSans.ttf, glyphs: 6253`;
a missing file panics with `pinned face not found`. Only
`font_discovery.rs` is about the system, and `just fonts` (in
`dev-shape`) runs it under `FULGURITE_REQUIRE_FONTS=1`.

**PPM, not PNG.** `Canvas::to_ppm` already writes it and it needs no
dependency. The PNG encoder lives in `fulgurite-agent`, which depends
on `fulgurite` with `features = ["observe"]` — not optional — and
Cargo unifies features across a selection, so a dev-dependency on it
from `fulgurite-shell/tests` would switch `observe` on for
`alloc_gate.rs`'s process and grow `NodeDesc` from 48 B to 72 B. Same
unification hazard M13's feature polarity is argued from.

### One test whose bound was computed from the wrong quantity

The red-pixel assertion first read the diff's body at a hardcoded offset
of 15; the header is 14 bytes, so every RGB triple was read one byte out
of phase and the count came back **0 red for 92 differences**. It found
itself immediately because the number printed beside the bound — in a
test written to catch exactly that shape.
