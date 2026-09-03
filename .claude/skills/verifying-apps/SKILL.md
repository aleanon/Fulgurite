---
name: verifying-apps
description: Use to verify that a lumen-static app, widget or style change behaves and looks correctly — headless frames, headless assertions, and driving the live window over the agent endpoint. Encodes the verification ladder, the implemented agent-method set, the no-auto-wait rule, the live-vs-headless measurer trap, and how to screenshot without a desktop tool. Applies whenever you changed app/widget/paint behavior and must prove it works, or are asked to test/verify/screenshot a lumen-static app.
---

# Verifying a lumen-static app

An agent must be able to verify its own work; that is the point of the
`observe` feature. There are four rungs; climb only as high as the change
demands. **Every rung asserts on state + semantics + bounds first, pixels
second** — a screenshot confirms, it does not prove.

| Rung | Command | Proves | Use when |
|---|---|---|---|
| 1. Headless frame | `just tally-frame 7` → `target/tally.ppm` → convert → `Read` | builds, lays out, paints | any visual change (cheapest sanity) |
| 2. Headless drive | `just tally-drive` / `just fixtures` | state, bindings, hit-testing by author id | always — this is the regression artifact |
| 3. Agent tests | `cargo test -p lumen-static-agent` | tree/layout/lint/changes as JSON | any change to what the agent can see |
| 4. Live window | `just run-agent` + `scripts/agent_client.py` | winit input, real fonts, the blit | new interactions; anything headless cannot catch |

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
*both* modes for this reason; do the same in any new example.

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
(`ui.getTree`, `ui.lint`, `ui.lastChange`), not to pixels.

## Rung 4 — the live window

```bash
just run-agent &                                   # window + JSON-RPC on 127.0.0.1:9230
python3 scripts/agent_client.py wait-port
python3 scripts/agent_client.py tree               # what the agent can see and do
python3 scripts/agent_client.py click '#inc10'
python3 scripts/agent_client.py layout '#value'    # → value, bounds, src
python3 scripts/agent_client.py screenshot /tmp/after.png
python3 scripts/agent_client.py quit
```
`Read /tmp/after.png`. `ui.screenshot` returns the exact RGBA the window
blits — no desktop screenshot tool, no unlocked session needed.

**Methods that exist** (`crates/lumen-static-agent/src/lib.rs`):
`ui.getTree` `ui.getLayout` `ui.resolve` `ui.probe` `ui.probeRegion`
`ui.lint` `ui.lastChange` `ui.lastDamage` `ui.waitSettled`
`ui.screenshot` · `input.click` `input.hover` `input.key` `input.type`
`input.scroll` `input.invokeAction` · `app.diagnostics` `app.logs`
`app.perf` `app.quit` (the last is the window's, not the crate's).

**Selectors:** `#id`, a kind name (`label`, `button`), `~text`,
`@file.rs:42`, or a table index. Not-found replies carry nearest misses;
ambiguity carries the candidates — read the error before retrying.
Repeater instances are not selectable; reach them via `ui.probe`.

**Actions do not auto-wait.** The reply's `frame` is the frame the
input was dispatched on; re-query (`layout`, `tree`, `screenshot`) after
acting. In the live window the RPC is answered on the event-loop thread
after `pump`, so a `layout` right after a `click` already sees the
result — but write the check anyway; it is what makes the run a test.

**Port lifecycle:** one window owns `LUMEN_AGENT_ADDR` (default
`127.0.0.1:9230`). A stale window from a previous run answers instead of
yours; `quit` it or set a different address. `wait-port` times out at
60 s — a longer wait means the build failed; read its log.

## What a green rung 4 looks like

The tally after `click #inc10`, `#inc`, `#inc`: `layout '#value'`
reports `"value": "12"`; the screenshot shows 12 in green with a
POSITIVE pill; `lint` is empty. That run is the acceptance test for
the window example and is what a change to fonts, painting, or input
must reproduce before it is called done.
