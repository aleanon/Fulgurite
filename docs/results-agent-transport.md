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
