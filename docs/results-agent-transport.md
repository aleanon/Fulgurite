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
