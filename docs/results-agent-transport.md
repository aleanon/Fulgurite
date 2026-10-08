# Results: M39 — what the agent endpoint answers

*M39, `docs/plan-agent-transport.md`. Owns every figure that plan and
`lumen_static_agent::transport` cite. Counts, not timings: nothing here
depends on the machine.*

## §1 The browser vectors (T39.0 → T39.1)

Each row is one fixture in `crates/lumen-static-agent/tests/browser/`,
replayed byte for byte on its own connection with the write side left
open, as a browser leaves it while it waits. **Captured** rows are what a
real headless browser sent, recorded by
`scripts/capture_browser_requests.py` from a page on another loopback
port: Firefox 156.0 and Chrome for Testing 145.0.7632.6, on 2026-09-25.
**Hand-written** rows are marked so; no browser API can send them.

Every body carries two JSON-RPC lines: a click on counter-window's
`#inc` and one on tasks-demo's `#spawn-one`.

"Before" is `b213ae0` (the base; both servers ran the copied loop).
"After" is T39.1's transport. The same numbers come out of the
display-free gate (`crates/lumen-static-agent/tests/transport.rs`,
`a_browser_request_executes_nothing`) and the event-loop gate
(`crates/lumen-static-winit/tests/agent_transport.rs`).

| vector | source | executed before | replies before, then | executed after | replies after, then |
|---|---|---|---|---|---|
| `fetch(.., {mode: 'no-cors', method: 'POST'})` | Firefox 156 | 2 | 17, held open | **0** | 1, closed |
| same | Chrome 145 | 2 | 18, held open | **0** | 1, closed |
| `<form enctype="text/plain">` POST | Firefox 156 | 2 | 18, held open | **0** | 1, closed |
| same | Chrome 145 | 2 | 20, held open | **0** | 1, closed |
| `navigator.sendBeacon` | Firefox 156 | 2 | 17, held open | **0** | 1, closed |
| same | Chrome 145 | 2 | 18, held open | **0** | 1, closed |
| `new WebSocket(..)` upgrade | Firefox 156 | 0 | 17, held open | **0** | 1, closed |
| same | Chrome 145 | 0 | 12, held open | **0** | 1, closed |
| `GET` with a JSON body | hand-written | 2 | 5, held open | **0** | 1, closed |
| HTTP/2 connection preface | hand-written | 2 | 4, held open | **0** | 1, closed |

**16 methods executed before, 0 after.** "Replies before" is one
`-32700` per request line and header, then one result per body line;
blank lines were skipped. "Held open" is the loop waiting for the next
line — the WebSocket rows executed nothing only because an upgrade has
no body, and the connection was still open to whatever came next.

The one reply after is `-32600` "this is a JSON-RPC line protocol, not
HTTP", because every vector's first line is an HTTP request line.

### Which mechanism carries it (T39.1's injections)

| injection | display-free gate | reading |
|---|---|---|
| `continue` on a refused line, HTTP check deleted | **red**: 2 executed on every body-carrying vector, connection held open 2 s | the base's behaviour, reproduced |
| HTTP check deleted only | green: first reply `-32700`, then closed | close-on-error alone refuses every vector |
| HTTP check kept, `continue` for everything else | green | the HTTP check alone refuses every vector, because each begins with a request line |

So each mechanism holds the gate by itself, and **close-on-error is the
one that does not depend on recognising HTTP**: a request line cannot be
a JSON object, since an HTTP method token cannot begin with `{`.

### Live

| where | before (`b213ae0`) | after (T39.1) | accept, same session |
|---|---|---|---|
| `scripts/live_gate.sh`, counter-window, Firefox fetch | `#value 0 -> 1`, 17 replies, held open | `#value 0 -> 0`, 1 reply, closed | a click: `#value 0 -> 1` |
| `scripts/tasks_drive.py`, tasks-demo (turnkey `run`), both strategies | `one=0 -> one=1`, 17 replies, held open (under the first injection above, which reproduces the base; the step did not exist there) | `one=0 -> one=0`, 1 reply, closed | step 1's click: `one=1` |

Both live steps first check with `ss -ltnp` that the port belongs to the
pid the script launched.

## §2 The bind (T39.2)

`transport::bind` resolves the address and refuses, with
`PermissionDenied` and a message naming `LUMEN_AGENT_ALLOW_REMOTE`, if
any resolved address is not loopback. From
`a_remote_bind_is_refused`:

| address | override | before (`b213ae0`) | after |
|---|---|---|---|
| `0.0.0.0:0` | no | bound (event-loop gate: `0.0.0.0:7813` bound and answered) | **refused** |
| `[::]:0` | no | bound | **refused** |
| `127.0.0.1:0` | no | bound | bound |
| `[::1]:0` | no | bound | bound |
| `localhost:0` | no | bound | bound (`127.0.0.1`) |
| `0.0.0.0:0` | `=1` | bound | bound, with a stderr warning that names the address and says there is no authentication |

Injection: the loopback filter made always-empty. Both refused rows
turned accepted ("0.0.0.0:0 -> accepted, bound 0.0.0.0:2335"), the
display-free gate failed by name on `0.0.0.0:0` and `[::]:0`, and the
event-loop gate's "a 0.0.0.0 bind is refused" went `FAIL ... bound and
answered: true`. The loopback rows stayed green.

## §3 Connections and lines (T39.3)

A thread per connection, up to `MAX_CONNECTIONS` = 16; a line read
through `take(MAX_LINE)`, `MAX_LINE` = 1 MiB. From `transport.rs` and the
event-loop gate:

| quantity | bound | before (`b213ae0`) | after |
|---|---|---|---|
| a second client, while a first connects and sends nothing | answered < 1000 ms | unanswered at 1000 ms (event-loop gate) | answered in 0 ms |
| heap growth, one line with no newline | < 2048 KiB | 32 MiB sent, **grew 32832 KiB**, connection held (event-loop gate) | 8 MiB sent, closed at the cap, grew 1207 KiB (display-free); event-loop gate grew 1089 KiB |
| a 17th concurrent connection | one `-32000` line | queued behind the first | `-32000` "too many agent connections"; the other 16 all answer; a slot comes back when one closes |

The growth after is the 1 MiB line buffer, which is the cap doing its
job, plus kilobytes from the tests running beside it.

Injections: (1) serve each connection inline instead of on a thread —
`an_idle_client_does_not_block_a_second` red, "answered: false in 1048
ms / bound 1000 ms"; (2) the bounded read replaced by an unbounded
`read_until` — `a_line_without_newline_is_capped` red, "sent 8192 KiB;
closed false; grew 8264 KiB / bound 2048 KiB".

## §4 The envelope (T39.4)

`hostile.rs` `the_envelope_is_validated`, one row per input, the code
printed beside the expected one:

| input | before (`b213ae0`) | after |
|---|---|---|
| `[]` | `-32601` "unknown method ``" | `-32600` |
| `[{..a click..}]` | `-32601` | `-32600` |
| `5` | `-32601` | `-32600` |
| `method: 5` | `-32601` | `-32600` |
| `jsonrpc: "1.0"` | a result | `-32600` |
| no `id` | a result (the click ran) | `-32600` |
| `params: "x"` | a result | `-32600` |
| `input.click` `x: 1e300` | a result, `at: [null, 0]` | `-32602` |
| `input.scroll` `dy: 1e39` | a result, `dy: null` | `-32602` |
| `ui.probe` `x: 1e300` | a result | `-32602` |
| `ui.explain` `at: [1e300, 0]` | a result, "a click at [inf,0]" | `-32602` |
| a well-formed `input.click` (accept) | a result | a result |

The wider accept direction is every other agent test in the workspace,
which already sent `jsonrpc: "2.0"` and an `id`: `cargo test --workspace`
green with the validation in place.

Injections: `unwrap_or("")` restored and the validation dropped — the
first seven rows red, `-32601` or a result where `-32600` was expected;
the finiteness check made always-true — the four coordinate rows red, a
result where `-32602` was expected.

## §5 The agent's own panic (T39.5)

`crates/lumen-static-shell/tests/agent_panic.rs`, a leaf whose
`observe` panics when armed (reached through `ui.getTree`) and whose
`layout` panics on the other arm (application code, reached through
`input.click`):

`Recover::On`/`Off` below are `Recovering`/`Crashing` since M52/T52.7 —
the policy became a type; the rows are unchanged, and `tests/agent_panic.rs`
runs both.

| case | before (`b213ae0`) | after |
|---|---|---|
| agent's panic, `Recover::On` | the test thread dies with the panic | `-32603` "the agent panicked answering `ui.getTree` at …agent_panic.rs:79:13", id echoed; the next `ui.getTree` answered |
| agent's panic, `Recover::Off` | the same | the same reply |
| application's panic, `Recover::Off` (accept) | unwinds out of the session | still unwinds, with its own message |
| application's panic, `Recover::On` (accept) | M36's: a result, `app.failure` names it | unchanged |

Injections: (1) the catch removed — both `-32603` rows red (the test
dies with "a widget's observe panicked on purpose"), and the
`Recover::Off` row red too, because the marker now escapes unwrapped;
(2) the re-raise removed — the `Recover::Off` row red, "the
application's panic was swallowed into a reply: {… -32603 …}".

## §6 The descendant step's error (T39.6)

`descendant.rs` `a_descendant_error_names_the_selector_once`; the
message must contain the whole selector exactly once and be shorter than
`4 × selector + 64` bytes:

| selector | before (`b213ae0`) | after |
|---|---|---|
| `#first >` | 134 B / bound 96 B, selector 15× (`#first >e#first >m…`) | 65 B, 1×: "descendant step `>` needs a selector on both sides" |
| `> #first` | "empty selector", 0× | 65 B, 1×, the same refusal |
| `#first >   ` | 179 B / bound 108 B, 15× | 68 B, 1× |
| `#first > e` | 64 B, 4× (`s#first > el#first > ector …`) | 45 B, 1×: "in `#first > e`: selector `e` matched nothing" |

Accept: every existing descendant test (`#first > #part-label`, chained
and unspaced steps) resolves as before, and `#first > #part-labl` still
reports `-32001` with its nearest misses (`part-label` among
`available_ids`). Injection: `m.replace(tail, selector)` restored beside
the empty-side check — the three empty cases stay green and `#first > e`
goes red, 4×, which is the case a fix guarding only the empty tail would
have left.

## §7 Every client, against the finished transport (T39.7)

T39.1–T39.6 in place, 2026-09-25, `DISPLAY=:0`, ports 7810–7819. The
injection column is the same sweep with the transport made to **close
after the first reply on every connection** — the check that the sweep
can see a transport that breaks clients.

| client | how it was run | finished transport | injected |
|---|---|---|---|
| `scripts/agent_client.py` + `scripts/ui_check.py` | `just live-gate` (counter-window) | **pass**: every check ok, and the browser replay `#value 0 -> 0` | fail: "agent endpoint closed the connection" at `centring` |
| the inline client in `scripts/agent_drive.sh` | `just agent-drive` (showcase, both strategies) | **pass**: palette, dropdown, press, shadow oracle "compared 29 frames, none diverged" | fail: `JSONDecodeError` on the second call's empty reply |
| the drive harness (`scripts/drive/harness.py`, `run.py`) | `just widget-drive`, both strategies | **pass**: "39 widgets, 94 actions … 0 failed" under `Everything` and under `Boundaries` | fail: "agent endpoint closed the connection" |
| `scripts/tasks_drive.py` | `just tasks-drive`, both strategies | **pass**: all five capabilities landed, and the replay spawned nothing | fail: `JSONDecodeError` |
| `scripts/hot_window_drive.py` | `just hot-window-drive` | **pass**: swaps in 0.44 s, 0.44 s, 0.64 s; failed build kept; panic shown | fail: `JSONDecodeError` |
| `scripts/panic_drive.py` + the inline client in `scripts/panic_drive.sh` | `just panic-drive` | **pass**: all seven sites caught under both strategies; release without `reload` still dies | fail: `JSONDecodeError` |

**No client relied on a malformed line being skipped** (plan §5's first
risk): every one sends one well-formed object per line with
`jsonrpc: "2.0"` and an `id`, and none sends a batch or a notification.
The injection turned all six red, not only the two the plan required
(`agent-drive` and `widget-drive`, which hold one connection for many
calls): every client makes more than one call on a connection.

## §8 A reply's latency (CI time, 2026-10-08)

**Every request cost about 41.6 ms, whatever it did** — a key press and
a full tree read alike. The reply was written with `writeln!` on the
bare `TcpStream`, which sends a `Value`'s `Display` output in the pieces
it produces and the newline apart; the first piece goes out, Nagle holds
the rest until it is acknowledged, and the client, which has nothing to
send until it sees the newline, acknowledges only when its delayed-ACK
timer fires. `send_line` writes each line in one `write_all`.

| measure | before | after |
|---|---:|---|
| `a_reply_is_not_held_for_an_ack`, median of 20 | 40.91 ms | 0.07 ms (bound 10 ms) |
| widget drive, four widgets (Button, Slider, Select, DataGrid) | 73 s | 11 s |
| widget drive, one full `Boundaries` leg | ~1 000 s | 158 s, 0 FAIL |

The leg was **6 247 s of the 9 236 s** `just ci` at `ac98b18`, six legs
at that rate. The Toast race fixed in `f3deb9a` was this latency too: a
dismissal timer expiring between two 40 ms calls.
