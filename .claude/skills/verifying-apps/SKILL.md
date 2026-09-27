---
name: verifying-apps
description: Use to verify that a lumen-static app, widget, style or renderer change behaves, looks and performs correctly — headless frames, headless assertions, the live window over the agent endpoint, and the measurable GUI-quality checks (centring, anti-aliasing, contrast, target size, latency). Encodes the verification ladder, the implemented agent-method set, the no-auto-wait rule, the headless-vs-live measurer trap, the performance budgets and how to profile. Applies whenever you changed app/widget/paint/input behavior and must prove it works, or are asked to test/verify/screenshot/profile a lumen-static app.
---

# Verifying a lumen-static app

An agent must be able to verify its own work; that is the point of the
`observe` feature. There are four rungs; climb only as high as the change
demands. **Every rung asserts on state + semantics + bounds first, pixels
second** — a screenshot confirms, it does not prove. **Performance is a
rung-4 check, not a separate activity**: a change that touches painting,
text or input is not verified until `ui_check.py latency` has passed.

| Rung | Command | Proves | Use when |
|---|---|---|---|
| 1. Headless frame | `just tally-frame 7` → `target/tally.ppm` → convert → `Read` | builds, lays out, paints | any visual change (cheapest sanity) |
| 2. Headless drive | `just tally-drive` / `just fixtures` | state, bindings, hit-testing by author id | always — this is the regression artifact |
| 3. Agent tests | `cargo test -p lumen-static-agent` | tree/layout/lint/changes as JSON | any change to what the agent can see |
| 4. Live window | `just live-gate`, or `just run-agent` + the scripts | real input, real fonts, the blit, **timing**, **GUI quality** | new interactions; paint/text/input; anything headless cannot catch |

## Rung 1 — a frame to look at

```bash
just tally-frame 12                               # examples/counter-window, --headless
python3 -c "from PIL import Image; Image.open('target/tally.ppm').save('/tmp/f.png')"
```
Then `Read /tmp/f.png`. Without PIL, `ffmpeg -i target/tally.ppm /tmp/f.png`.

**Trap: headless and live use different measurers unless you say
otherwise.** `Host::pump()` measures with `BlockMetrics` (one block per
glyph); the window measures with `FontMetrics` (fontdue, `truetype`
feature). A headless frame that looks right can still wrap in the
window. `counter-window` passes its `FontMetrics` to `pump_with` in
*both* modes for this reason; do the same in any new example. The same
trap applies to the agent's audit: install the host's measurer with
`Agent::set_text_measure`, or `ui.lint` reports the stand-in's opinion.

## Rung 2 — drive headlessly and assert

`examples/counter-window --drive` is the pattern: click by author id via
`ui.getLayout`, one `pump` per click, then assert on `host.app.<field>`
and the bound label's `as_str()`. `examples/counter` is the minimal
form. Both are run by `just fixtures`, which `just test` includes — an
assertion in `main` that nothing runs is not a test.

## Rung 3 — the agent surface in tests

`crates/lumen-static-agent/tests/*.rs` show the fixture shape:
`TreeRef::of_root(&app, &styles, &states)` + `agent.dispatch(&tree,
&json!({..}))`. Use it when the change is to what an agent sees
(`ui.getTree`, `ui.lint`, `ui.lastChange`, `app.perf`), not to pixels.

## Rung 4 — the live window

**`just live-gate` is the automated form**: opens the window on its own
port, warms it, runs every check in `scripts/ui_check.py`, closes it,
and exits non-zero on any failure. `just check` runs it through `just
live`, with every other live gate, and **fails without a display** (exit
77) unless `LUMEN_ALLOW_SKIP=1` — a skip is not a pass (M43/T43.13). Run
it before claiming a paint/text/input change is done.

By hand, for anything the gate does not cover:

```bash
just run-agent &                                   # window + JSON-RPC on 127.0.0.1:9230
python3 scripts/agent_client.py wait-port
python3 scripts/agent_client.py tree               # what the agent can see and do
python3 scripts/agent_client.py click '#inc10'
python3 scripts/agent_client.py layout '#value'    # → value, bounds, src
python3 scripts/agent_client.py screenshot /tmp/after.png
python3 scripts/agent_client.py call app.perf      # timings, p50/p95 over 128 frames
python3 scripts/ui_check.py all                    # or one of: centring edges targets contrast presented press latency lint
python3 scripts/agent_client.py quit
```
`Read /tmp/after.png`. `ui.screenshot` returns the renderer's canvas —
no desktop screenshot tool, no unlocked session needed.

**But the canvas is not the screen.** Everything between them — the
blit, the damage rect, the present, the surface buffer's age — is
downstream of every other method here, and a bug there is invisible to
all of them: on 2026-09-04 another window's pixels were on screen while
`ui.screenshot`, `ui.lint` and every check reported a perfect frame.
`ui.screenshot { source: "surface" }` reads the window back from the
display server, and `ui_check.py presented` compares the two. **Run it
after any change to presenting, damage or the blit** — and note that
read-back exists on X11 and Web only, so elsewhere it says "not
checkable", which is not a pass.

**Methods that exist** (`crates/lumen-static-agent/src/lib.rs`, and
`crates/lumen-static-winit/src/lib.rs`'s `answer_agent`; this list is
checked against both by `scripts/check_agent_methods.sh`, M48/T48.14):
`ui.getTree` `ui.getLayout` `ui.resolve` `ui.probe` `ui.probeRegion`
`ui.explain` `ui.getAppliedStyles` `ui.lint` `ui.lastChange`
`ui.lastDamage` `ui.waitSettled` `ui.screenshot` · `input.click` `input.pointerDown` `input.pointerUp`
`input.hover` `input.key` `input.type` `input.fill` `input.paste` `input.compose`
`input.scrollIntoView` `input.scroll` `input.invokeAction` ·
`app.diagnostics` `app.logs` `app.perf` `app.setPalette` `app.shadow`
`app.failure` `app.retry` · the window's own: `app.quit` (every turnkey
window since M48/T48.13; the reply comes first, then the process exits 0)
and `app.reloads`. `input.compose` takes `{text, commit}` or
`{cancel: true}` and routes through the same `Host::compose` the window's
IME path does.
`input.key` takes `mods` (`["ctrl"]`, held for that key only) and
`input.click` takes `clicks` (2 or 3) — M45/T45.6.

**A node says what a screen reader would announce** (M48/T48.5): beside
`id`, `role`, `value`, `label`, `bounds`, `children`, `src` and `wid`,
`ui.getTree` carries — only when set — `toggled` (`"true"`/`"false"`/
`"mixed"`), `numeric` (`value`, `min`, `max`, `step`), `expanded`,
`modal`, `hidden`, `disabled`, `busy` and `actions`. An unchecked box
reads `toggled: "false"`, never an absent field; `ui.resolve` and
`ui.getLayout` carry the same fields (T48.6).

**When a click does not do what you expected, ask `ui.explain` before
reading code.** It runs the *real* hit test at the node's centre and
reports what came back — occluded by which node, zero area, hidden, not
focusable — so it cannot disagree with what a click actually does. Pair
it with `ui.getAppliedStyles` when the question is visual rather than
positional: that reports the cascade's answer *and* the painted
background, so a darker button in a screenshot resolves to "pressed"
rather than to a guess about the stylesheet.

**Selectors:** `#id`, a kind name (`label`, `button`), `~text`,
`@file.rs:42`, or a table index. Not-found replies carry nearest misses;
ambiguity carries the candidates — read the error before retrying.
Repeater instances are not selectable; reach them via `ui.probe`.

**`input.type` inserts at the caret; `input.fill` replaces a value.**
There is no selection and no select-all (a routing fact — see
`docs/text-and-scrolling-decisions.md` B-cluster), so clearing a field by
hand is one `Backspace` per character: 23 RPCs for a 20-character value.
`input.fill` is one. It refuses rather than typing when the target is not
where a click would land, because a fill that misses sends a run of
backspaces to whatever *does* hold focus — destructive and silent. When
it refuses, `input.scrollIntoView` is usually the answer: a control
scrolled out of its viewport cannot be clicked at all, since a selector
resolves to the node's centre and that centre is a point the hit test
correctly rejects.

**Actions do not auto-wait.** The reply's `frame` is **the first
journal frame that includes this input; ask with `since = frame - 1`**
(M48/T48.1) — `ui.lastChange` and `ui.lastDamage` answer from that frame
once the window has drawn it. `host_frame` is the host's own counter, for
debugging only: it moves twice per input and means nothing to `since`.
Re-query (`layout`, `tree`, `screenshot`) after acting. **`ui.waitSettled`
answers from the host** (M48/T48.2): `settled` is false while the last
frame animated, the inbox has a backlog or holds a result, a spawned job
is in flight (`pending_tasks`) or a timer is due within a frame, and the
reply names each term; running subscriptions are counted
(`subscriptions`), not waited on. `settled: null` means the host cannot
say — not "yes". It does not block: poll it. In the live window the RPC is answered on the event-loop thread
after `pump`, so a `layout` right after a `click` already sees the
result — but write the check anyway; it is what makes the run a test.

**Port lifecycle:** `LUMEN_AGENT_ADDR` is **the one name** for the
address (M48/T48.11): every turnkey window reads it in
`lumen-static-winit`, overriding the program's own `Program::agent`
default (showcase's is `127.0.0.1:7654`; with neither set there is no
endpoint), and the client and drive scripts read it too. `LUMEN_AGENT`
still works for one release and prints a deprecation line.
One window owns `LUMEN_AGENT_ADDR` (default
`127.0.0.1:9230`; the gate uses `:9231` so it never talks to your dev
window). **A window whose endpoint cannot bind exits with status 3**
(M42/T42.1), naming the address — it used to keep running without an
endpoint while a stale window from a previous run answered instead of
yours. `quit` the stale one or set a different address. **Ask which
process answered**: `app.diagnostics` carries a `process` block —
`pid`, `exe`, `strategy` (the host's invalidation type, e.g.
`lumen_static::invalidate::Boundaries<256>`) and `features`
(`agent`, `everything`, `shadow-oracle`, `reload`) — or
`{"supported": false}` from a host that cannot say, which is not an
answer. The live drives check it on every leg (`scripts/live/launch.py`). `wait-port` times out at 60 s — a
longer wait means the build failed; read its log. **A malformed line ends
the connection** (M39): anything that is not one JSON object per line —
an HTTP request, a batch, a stray `5` — gets one error reply and a close,
so a hand-typed request that fails to parse means reconnecting, not
retyping. That is what keeps a web page from driving the window. The
address must be **loopback** (`127.0.0.1`, `[::1]`, `localhost`): a
`0.0.0.0` bind is refused with `agent: cannot bind ...` unless
`LUMEN_AGENT_ALLOW_REMOTE=1`, which binds it and warns that the endpoint
has no authentication. **One window serves up to sixteen clients at
once** (M39), so a harness holding a connection open no longer blocks a
second tool; requests from two clients interleave on the event loop, so
a frame number one client reads may have been advanced by the other. A
line over 1 MiB ends the connection. Every request needs `"jsonrpc":
"2.0"` and an `id` — one without is `-32600`, not a notification — and a
coordinate must be finite and within ±1e7.

## Performance — budgets, and how to find out where the time goes

`app.perf` returns, per frame and as p50/p95/max over the last 128:
`layout_ns`, `paint_ns`, `render_ns` (display list → pixels),
`present_ns` (pixels → surface), `frame_ns` (the whole redraw),
`input_latency_ns` (oldest input consumed → that frame presented) and
`damaged_px` (how much of the frame was actually redrawn, in physical
pixels since M40/T40.1 — `ui.lastDamage` is logical and is not the same
quantity).
It also carries `atlas_give_ups`, once, beside `last`: the GPU backend's
running count of frames drawn with glyphs or images missing because an
atlas could not hold them (M51/T51.1). Non-zero is a visible defect;
`null` means the frames came from the software canvas, which has no atlas.
`app.perf { since: F }` adds what the surface presented over the journal
frames after F (`since.damaged_px`, including the frames an agent's own
request presented) — the drive prints it beside `ui.lastDamage`'s ratio
(M48/T48.4). `input_latency_ns` is the number a person feels; `damaged_px` is the
one that explains a slow frame, and the one that catches a partial-redraw
path that has quietly degraded to full frames — every *stage* still
looks reasonable when that happens.

| budget | value | why |
|---|---|---|
| input → presented, p95 | ≤ 100 ms | Nielsen's "feels instantaneous"; below ~50 ms nobody can tell |
| frame, p95 | ≤ 16 ms | one 60 Hz refresh |
| layout + paint | ≤ 100 µs | the static tree's promise; anything more is a memo missing |
| `damaged_px` p50 | ≪ viewport | a partial redraw that damages the whole frame is not one |

**A stage nobody measured is `null`, and `null` fails** (M48/T48.3). Every
turnkey window (`lumen_static_winit::run`, a `Program`) reported 0 ns for
every stage until then, and the budgets above passed on the zeros.
`ui_check.py latency` now prints `NOT MEASURED` and fails on a window with
no samples, a `null` stage or a frame that took 0 ns; `ui_check.py
selftest` shows it both ways. A turnkey window does not measure
`input_latency_ns` (counter-window does), so there it is `null`.

Read the *warm* window: the first ~5 frames after launch are 4× slower
(cold clocks, first touch of the pixel buffer). `ui_check.py latency`
clicks before it reads; the gate warms with 20 clicks.

When a budget fails, **profile before guessing**: `just profile` prints
the median cost of each stage and each command class headlessly. The
2026-09-03 round found layout at 1 µs and one command class at 97% of
a 190 ms frame; the fix was in the renderer, not the tree
(`docs/results-frame-cost.md`). If the headless profile is fast and the
live frame is slow, the difference is the build profile
(`Cargo.toml`: the shell is optimised even in dev — check that is still
true) or the surface.

## GUI quality — the checks, and the theory behind them

Unless the task says otherwise, a screen is held to these. Each is a
number `scripts/ui_check.py` produces from `ui.getLayout` + `ui.screenshot`
+ `app.perf`, and a budget; the budgets are the usual ones from the
literature, cited so the next person can disagree with a source rather
than with a script.

| check | rule | source |
|---|---|---|
| `centring` | text ink centred in its control, ±2 px, both axes — cap height, not line box (`text::cap_shift`) | optical/"vertical trim" centring, every design system |
| `edges` | rounded corners are anti-aliased (blended pixels on the arc) | no stair-steps at 1× |
| `targets` | pointer targets ≥ 24×24 px (44 is the comfortable figure) | WCAG 2.5.8; Apple HIG 44 pt |
| `contrast` | text vs its background ≥ 4.5:1 (3:1 for ≥ 18.7 px bold) | WCAG 1.4.3 AA |
| `presented` | what is on screen matches what was drawn (X11/Web only) | the canvas is not the screen; everything after it is otherwise unobservable |
| `press` | a control changes while held, and restores | every platform's HIG; the affordance people check without noticing |
| `latency` | budgets above | Nielsen 1993; 60 Hz |
| `lint` | the agent's ambient audit is empty (blank, truncated, collapsed, hidden-but-hit) | `crates/lumen-static-agent/src/audit.rs` |

`press` holds the button open — `input.pointerDown`, screenshot,
`input.pointerUp` — because `input.click` queues both halves into one
pump and the frame it returns has already let go. That is the shape any
"state during a gesture" check needs.

Beyond the script, also look at
the screenshot for: consistent spacing on an 4/8 px rhythm, one visual
hierarchy (size/weight/colour agree on what matters most), no text
truncation, no layout shift between frames that should be identical
(`ui.lastChange` after a no-op frame must be empty).

## What a green rung 4 looks like

`just live-gate` prints `all checks passed`. By hand: after `click
#inc10`, `#inc`, `#inc`, `layout '#value'` reports `"value": "12"`; the
screenshot shows 12 in green with a POSITIVE pill; `app.perf` shows
input → presented under 10 ms; `lint` is empty. That run is the
acceptance test for the window example and is what a change to fonts,
painting, or input must reproduce before it is called done.
