# M39 — the agent endpoint answers only what speaks its protocol

> **T39.0–T39.7 DONE 2026-09-25.** Every figure is in
> `docs/results-agent-transport.md`.
> - T39.0: gates first, red at the base; fixtures captured from a real
>   Firefox 156 and Chrome 145 (16 methods executed by 10 vectors).
> - T39.1: one transport in `fulgurite_agent::transport`, closing on
>   any line that is not a JSON object; 0 executed; a static check
>   refuses a second listener.
> - T39.2: loopback only unless `FULGURITE_AGENT_ALLOW_REMOTE=1`.
> - T39.3: a thread per connection (16), a 1 MiB line cap; a refusal
>   half-closes and drains so its reply arrives.
> - T39.4: envelope validated at dispatch; coordinates finite within
>   ±1e7.
> - T39.5: the agent's own panic is `-32603`; the application's still
>   unwinds under `Recover::Off`.
> - T39.6: a descendant error names the selector once.
> - T39.7: all six clients pass against the finished transport, and all
>   six fail under a close-after-first-reply injection.
>
> §6 records what building found.
>
> **Written 2026-09-24.** Closes R14-F1 (S0), R14-F2,
> R14-F3, R14-F5, R14-F6 and R14-F7, from M38's deep review
> (`docs/results-deep-review.md`) as re-triaged against `1717808` on
> 2026-09-24. **Phase 1**, in parallel with M40. It depends on nothing
> queued. M48 waits for T39.1: no new agent method lands before the
> transport refuses a browser.
>
> The ask, from the review: *a web page the developer happens to have
> open must not be able to drive the application under development.*
> Today it can. The endpoint reads newline-delimited JSON-RPC, answers
> every line that fails to parse with `-32700`, and keeps reading. An
> HTTP request is a request line, some headers and a blank line, each
> of which fails to parse, followed by a body the page chose. A
> `text/plain` `no-cors` POST is a CORS simple request, so no preflight
> stops it. The page cannot read the reply. It does not need to:
> `input.click` has already run.
>
> **The re-triage reproduced it on both servers at `1717808`**, the
> turnkey one in `fulgurite-winit` and counter-window's own copy.
> M37 did not change the loop; it moved the tree behind `Session`, so a
> hot window's endpoint now drives whichever library loaded last, and
> M36's `app.retry` lets a page clear a panic screen and re-run the
> application. It is still UI control, not code execution: no agent
> method writes a file, starts a build or loads a library.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **One transport.** Two servers carry the S0 today because the loop
   was copied. After T39.1 there is one loop, in the agent crate, and a
   static check refuses a second.
2. **The refusal is framing, not policy.** Nothing inspects an
   `Origin` or a `Host` header, because nothing reads headers at all: a
   line that is not a JSON object ends the connection. Every browser
   path (fetch, a form, a beacon, a WebSocket upgrade, DNS rebinding)
   starts with an HTTP request line, so nothing after it runs.
3. **Both directions, in the same session** (AGENT.md rule 3). Each
   refusal gate has a well-formed client beside it that must still be
   answered. The milestone does not close until every existing client
   has run green against the finished transport (T39.7).
4. **The gate replays what a real browser sends, byte for byte.** A
   request typed from memory tests the author's idea of a browser. The
   bytes are captured once from a real one and committed as a fixture.

## 1. What is wrong, at `1717808`

### 1.1 A parse error is skipped, so an HTTP body executes (R14-F1, S0)

`crates/fulgurite-winit/src/agent.rs:61-97` (`serve`) and
`examples/counter-window/src/agent_socket.rs:121-160` (`serve_agent`)
are the same loop. Each line that fails `serde_json::from_str` gets a
`-32700` reply and `continue`; the next line is read on the same
connection. The re-triage sent both servers the bytes of a `no-cors`
POST (`Origin: http://evil.example`, `Content-Type: text/plain`, a
JSON body line). Both answered six `-32700`s, one for the request line
and one per header, and then **executed the body**: counter-window's
tally moved, and tasks-demo's `#counts` moved. The review's own run
used a real headless Firefox page (R14-F1; `docs/results-deep-review.md`).
Blank lines are skipped by `line.trim().is_empty()` (`agent.rs:74`,
`agent_socket.rs:137`). So the six replies are the request line plus
five headers, not "one per header line and the blank separator" as the
re-triage worded it.

### 1.2 Whatever address is asked for is bound (R14-F2, S2)

`agent.rs:63` and `agent_socket.rs:123` call `TcpListener::bind(&addr)`
on whatever string they are handed. `0.0.0.0:<port>` serves the tree
and every `input.*` method to the LAN, and the only output is
`agent: listening on 0.0.0.0:<port>`. The re-triage reproduced it at
`1717808` against tasks-demo. It is opt-in (the environment variable
must be set), which is why the verifiers moved it from S1 to S2.

### 1.3 One connection at a time, and no line cap (R14-F3, S2)

Both loops are `for stream in listener.incoming()` with
`BufReader::lines()` (`agent.rs:71-73`, `agent_socket.rs:131-136`).
That means one client at a time, served to EOF, with no bound on a
line. An idle client locks every other client out. The re-triage's
second client timed out at its 3 s limit and was answered at once
after the first closed. A line with no newline grows the process with
the bytes sent (R14-F3). Note that `scripts/agent_client.py`'s
`wait_for_port` opens and closes a probe connection, and a harness
that holds an AgentClient open blocks every other tool until it
exits.

### 1.4 The envelope is not validated (R14-F7, S3)

`Agent::dispatch` and `Agent::dispatch_input` take the method as
`as_str().unwrap_or("")` (`crates/fulgurite-agent/src/lib.rs:395`,
`:413`). So a batch, a bare number, or a non-string method falls
through to `METHOD_NOT_FOUND` with `unknown method ```. A request with
no `id` is answered. `jsonrpc: "1.0"` is accepted. Coordinates are
cast `as f32` with no finiteness check at four sites: `point_param`
(`:760`), `input.scroll`'s `dy` (`:510`), `ui.probe` (`:1074`) and
`ui.explain`'s `at` (`:912`). So `x: 1e300` is answered as a success
with `at: [null, null]`. M36 put `app.failure`, `app.retry` and
`app.shadow` ahead of the fallthrough (`:414-435`) and added no
envelope check.

### 1.5 The agent's own panic ends the process (R14-F5, S3, PARTIAL)

M36 guards the application code the agent reaches: `Live::pump`
(`crates/fulgurite-shell/src/session.rs:480-486`, through
`run_guarded`) and `agent_frame_done` (`:422-433`). But
`Agent::dispatch_input` itself is not guarded. `Session::agent`
(`session.rs:395-412`) calls it bare, and it is reached from
`answer_agent` (`crates/fulgurite-winit/src/lib.rs:643`). That call
covers selector parsing, `get_tree`, `ui.explain` and every widget's
`observe` impl. The review's injection (`e00c288`, a panic in the
`ui.waitSettled` arm) exited the process with status 101. No real
panic was found in 1,250 hostile requests (R14-F5), which is why it
is S3.

### 1.6 A trailing `>` garbles the error and amplifies it (R14-F6, S3)

`resolve_one`'s descendant split reports a failure as
`m.replace(tail.trim(), selector)` (`lib.rs:1909`). If the tail is
empty, `str::replace` inserts the whole selector between every
character of `empty selector`. The re-triage measured a 1,079 B
request drawing a 15,192 B reply, built on the event-loop thread.
**The empty tail is not the only case.** A short non-empty tail is
replaced wherever it occurs in the message: `#x > e` substitutes the
selector for every `e` in `selector ... matched nothing`. So guarding
only the empty tail, the first fix the re-triage offers, would leave
the garbling in place.

## 2. Decisions

### 2.1 What refuses a browser

| option | stops | breaks | verdict |
|---|---|---|---|
| **(a) close on any line that is not a JSON object**, with one `-32600` "this is a JSON-RPC line protocol, not HTTP" reply when the line looks like an HTTP request line | every browser path, including DNS rebinding and a public origin, whatever the browser's local-network policy, because each one begins with a request line | no client. All eight send one well-formed object per line (T39.7 lists them). The only change in behaviour is that a malformed line ends the session instead of being skipped | **chosen** |
| (b) a per-session token, written 0600 to the runtime directory and required in every request | the above, and **other users** on a shared host | all eight clients until each reads the file; the skill needs a paragraph | **deferred** (§4) |
| (c) loopback-only binding | a LAN peer | nothing | **needed but not sufficient**: the browser is on loopback too. Ships as T39.2 for R14-F2 |
| (d) a Unix domain socket | browsers and other users, by file mode | every client (`create_connection` goes), Windows differs, and the `host:port` environment variable stops meaning anything | rejected |

**(a) is load-bearing, and the HTTP check is only a message.**
Closing on a parse error already refuses the request line, because
`POST / HTTP/1.1` is not JSON. The HTTP-shaped check exists so a
developer who points `curl` at the port learns why it closed. T39.1's
injections show this: each mechanism is removed separately, to show
which one carries the gate.

A request line cannot be a JSON object. HTTP's method token excludes
`{` and `"`, so no browser API can begin a connection with a JSON
line. That is the whole argument, and T39.1's fixture table checks it
for each vector rather than trusting it.

**What (a) does not defend against:** another process running as the
same user. That process can already `ptrace` the application or kill
it, so a token would buy nothing there. What a token *would* buy is
isolation from other users on the same host. §4 names the condition
that reopens it.

### 2.2 Where the transport lives

In `fulgurite-agent`, as a plain module and **not behind a
feature**. The crate is already `std` (it depends on `serde_json`), and
the linker drops an uncalled `serve`, as it drops everything else a
binary does not name (`docs/results-widget-crate.md`). A feature would
be another seam an application can forget to turn on, and R12-F3
(M48) is a finding about exactly such a seam.

The loop is generic over the step that reaches the tree: a handler
takes a request and returns its reply. The turnkey window's handler
sends an `Rpc` through its proxy, and counter-window's sends its own
`UserEvent::Agent`. Both keep the thread rule: no thread but the
loop's touches the tree (`agent.rs` module doc). The transport never
sees the tree, only JSON.

### 2.3 Connections and lines

- **A thread per connection**, up to 16 at once. The 17th gets one
  `-32000` line ("too many agent connections") and is closed. Every
  request is already serialised through the event loop, so concurrent
  connections share no new state.
- **No idle timeout.** The re-triage suggested one. It would cut a
  legitimate slow session and adds nothing a thread per connection
  does not already give: an idle client costs a thread, not the other
  clients' service. Holding 16 idle connections to lock the endpoint
  out needs a local process, and §2.1 already puts those out of scope.
- **A 1 MiB line cap**, where the re-triage suggested 16 MiB. The
  largest legitimate request is `input.type` at `MAX_TYPE_CHARS`
  (`lib.rs:297`), a few tens of KiB even fully escaped. At the cap the
  server sends `-32600` "line exceeds 1 MiB" and closes. So the worst
  case is bounded at 16 × 1 MiB.

### 2.4 Batches and notifications are refused

A batch (an array) is not an object, so rule (a) closes it, after a
`-32600` "batches are not supported". No client sends one, and a batch
of `input.*` calls would be dispatched with no frame between its
steps, which no caller could reason about. A request with no `id`
gets `-32600` rather than silent execution. The spec says a
notification gets no reply, but a write method that runs and says
nothing is the worse surprise, and no client uses notifications.

Envelope validation runs **at dispatch**, in one function in the agent
crate, called at the top of both `Agent::dispatch` and
`Agent::dispatch_input`. The re-triage put it in the transport. There,
`hostile.rs`, `TestApp` and `Recording::replay`, which call dispatch
in-process, would never exercise it, and the gate that tests it lives
at dispatch level. The transport handles framing only: parse, the
object check, and the cap.

### 2.5 The agent's own panic is a reply, and the application's is not swallowed

`Session::agent` wraps `Agent::dispatch_input` in `catch_unwind`
whenever `panic = "unwind"`, whatever the `Recover` value. The reply
is `-32603` "the agent panicked answering `<method>`", with the
location from M36's hook, and the endpoint keeps serving.

**One thing the re-triage's fix got wrong, in each direction.**

- **No journal reset is needed.** `Agent::dispatch_input` takes
  `&self`, so a panic inside it cannot leave the journal half-written.
- **An application panic must not be caught here.** Under
  `Recover::Off` (a release build without `reload`,
  `crates/fulgurite-winit/src/lib.rs:248-251`), `Live::pump` runs
  application code unguarded. An outer `catch_unwind` around dispatch
  would catch that panic and keep a half-updated host running, which
  breaks M36 rule 3 (`docs/plan-panic-recovery.md`): a shipped
  application crashes where a crash reporter can see it.

So `Live::pump` marks the call as application code for its duration.
The outer catch re-raises a panic it finds marked, with
`resume_unwind`. Under `Recover::On` the pump's own guard has caught
the panic first, so nothing marked ever reaches the outer catch.

## T39.0 — the gates first, red at `1717808`

Every finding becomes a check that fails today, and each is committed
with its red output in the commit body, as M35/T35.0 and M36/T36.0
did.

1. **Capture a browser's bytes.** A scratch page runs
   `fetch('http://127.0.0.1:<port>/', {method: 'POST', mode: 'no-cors',
   body: '<one JSON-RPC line>\n'})` in a real headless browser. It
   posts to a listener that records the raw bytes to a file. Commit the
   file under `crates/fulgurite-agent/tests/`, with the browser and
   its version in a header comment. Capture three more the same way:
   a `<form enctype="text/plain">` POST, whose `name=value` body can be
   shaped into a JSON line (the classic trick); a
   `navigator.sendBeacon`; and a `new WebSocket(..)` upgrade. Write two
   by hand and label them hand-written: a GET with a JSON line after
   the blank line, and an `HTTP/2` connection preface. **Only the
   captured ones are "byte for byte".**
2. **The turnkey server, through a real event loop.** Add
   `crates/fulgurite-winit/tests/agent_transport.rs`,
   `harness = false`, `--features agent`, needing a display. This is
   the review's scratch `f927617` shape. It runs `agent::serve` against
   a handler that counts executed methods, and plays each fixture on
   its own connection. **Red today:** `methods executed: 4 / bound 0`
   (the four captured vectors; the exact count goes in the commit
   body). The same binary carries R14-F2's `0.0.0.0` bind (red: bound
   and answered), R14-F3's idle-client case (red: the second client is
   unanswered at the 1 s bound), and the unterminated line (red: the
   process grows by the flood size, printed against a 2 MiB bound).
3. **Counter-window, live.** A step in `scripts/live_gate.sh` that
   sends the captured POST and reads the tally before and after. **Red
   today:** `tally 0 -> 10`. It is live because that server is the
   example's own until T39.1. It checks that the port is owned by the
   pid the script launched (`ss -ltnp`), because M42's harness identity
   lands after this milestone.
4. **Envelope, dispatch level** (R14-F7). Add a table-driven
   "the_envelope_is_validated" to
   `crates/fulgurite-agent/tests/hostile.rs`. The inputs: `[]`,
   `[{..}]`, `5`, `{"method": 5}`, `jsonrpc: "1.0"`, no `id`,
   `x: 1e300`, `dy: 1e39`, `ui.probe` with `x: 1e300`, and `ui.explain`
   with `at: [1e300, 0]`. It prints `input -> code (expected)` for
   each. **Red today:** `-32601` where `-32600` is expected, and a
   result where `-32602` is expected. A well-formed `input.click` in
   the same table is the accept row.
5. **The agent's own panic** (R14-F5). This needs a method that panics
   on request, compiled only for tests, as M36/T36.0 did with its panic
   fixture. A `fulgurite-shell` test drives it through
   `Session::agent`. **Red today:** the test binary dies with the
   panic, instead of receiving a `-32603` reply.
6. **The descendant step** (R14-F6). Add "a_descendant_error_names_the_selector_once"
   to `crates/fulgurite-agent/tests/descendant.rs`. The cases are
   `#x >`, `> #x`, `#x >   ` and `#x > e`. It asserts the message
   contains the whole selector exactly once and is shorter than
   `4 × selector + 64` bytes, printing `reply N B / request M B`.
   **Red today** on all four. The fourth case is the one a fix that
   only guards the empty tail would leave red.

Every gate above is committed marked expected-to-fail until the task
that closes it lands, and the commit body carries each one's red
output.

## T39.1 — one transport that closes on anything but a JSON object

**Closes:** R14-F1.

**Change.** A transport module in `crates/fulgurite-agent/src/`,
per §2.1–§2.2. It has a bind step (T39.2 adds the policy) and a serve
loop generic over the handler. It reads a line; skips a blank one; and
if the line does not parse as a JSON **object**, writes one reply and
closes. That reply is `-32600` naming HTTP when the line starts with an
HTTP method token or contains ` HTTP/`, and `-32700` or `-32600`
otherwise. `fulgurite_winit::agent::serve` becomes a thin call into
it, with a proxy-forwarding handler. `examples/counter-window` deletes
its loop and calls the same function with its own handler.
`Session::agent`'s own `-32700` path (`session.rs:396-405`) stays, for
a caller that is not the transport.

**Gate.**

- **Display-free.** Add `crates/fulgurite-agent/tests/transport.rs`,
  with "a_browser_request_executes_nothing". It binds `127.0.0.1:0`,
  plays every T39.0 fixture on its own connection against a counting
  handler, reads to EOF with a 2 s timeout, and asserts that nothing
  executed and that the connection closed. It prints
  `vector: methods executed N / bound 0; replies before close K; closed after X ms`.
- **Accept direction, same file.** "a_line_client_is_answered" sends
  two well-formed lines on one connection and asserts two executions
  and two replies with their ids echoed.
- **T39.0's winit test** goes green unchanged. That is the assembly
  check that the turnkey server really uses the transport.
- **The live step** in `scripts/live_gate.sh` reads `tally 0 -> 0`,
  followed by an ordinary click that moves it (accept). Add the same
  pair to `scripts/tasks_drive.py` against `#spawn-one`, so the
  turnkey window is driven live too.
- **One server.** A `just static` check asserts that the workspace has
  exactly one call that binds a TCP listener outside tests, and that it
  is in the transport module. It prints each site it finds.

**Injection.**

1. Restore `continue` on a parse error and delete the HTTP check. The
   display-free test goes red with `methods executed: 1 / bound 0` on
   each captured vector.
2. Delete **only** the HTTP check. It stays green, because the request
   line fails to parse and closes the connection. Record this as the
   evidence that close-on-error carries the gate.
3. Keep the HTTP check and restore `continue` for everything else. It
   stays green on every vector, because each one's first line is an
   HTTP request line.

2 and 3 together show that each mechanism holds the gate on its own.
For the static check, re-add counter-window's loop: the check names
`agent_socket.rs`.

**Docs in the same commit.**

- `docs/00-handoff-static-tree.md` §9 "Agent observability" gains the
  threat model (a local line protocol that refuses HTTP; same-user
  processes out of scope; the token deferred, and why) as a
  **DECIDED** item pointing at the new results doc.
- A new results doc, `results-agent-transport.md`, holds one row per
  vector with executed-before and executed-after. **Its path goes into
  `scripts/doc-paths.allow` until this commit writes it.**
- The `fulgurite-winit` `agent` module doc and `Config::agent`'s
  rustdoc (`lib.rs:191-203`).
- The header of `crates/fulgurite-agent/tests/hostile.rs`: "an agent
  is untrusted input" is now true of the socket, not just of dispatch.
- The port-lifecycle paragraph of `.claude/skills/verifying-apps/SKILL.md`:
  a malformed line now ends the connection.
- The book's "JSON-RPC over an already-laid-out tree" section
  (`book/src/agent.md`): one paragraph on what the endpoint refuses.
- AGENT.md's queue entry.

**Effort:** M. **Depends on:** T39.0.

## T39.2 — loopback only, unless overridden

**Closes:** R14-F2.

**Change.** The transport's bind step resolves the address and refuses,
with a message naming the override, if any resolved address is not
loopback. The override is the `FULGURITE_AGENT_ALLOW_REMOTE=1` environment
variable. With the override set, it prints a warning that names the
interfaces and says the endpoint has no authentication. `localhost` is
accepted because it resolves to loopback. Every script and every
documented address already binds `127.0.0.1`.

**Gate.** Add "a_remote_bind_is_refused" to `transport.rs`. It runs the
bind step over `0.0.0.0:0` and `[::]:0` (refused), and over
`127.0.0.1:0`, `[::1]:0` and `localhost:0` (accepted: the accept
direction). It also runs `0.0.0.0:0` with the override (accepted). It
prints `address -> verdict` for each. T39.0's winit `0.0.0.0` case
goes green.

**Injection.** Make the loopback check always pass: the test fails on
`0.0.0.0` by name, and the loopback rows stay green.

**Docs in the same commit.** `Config::agent`'s rustdoc, the skill's
port-lifecycle paragraph, the results doc, and the handoff §9 entry.

**Effort:** S. **Depends on:** T39.1.

## T39.3 — a thread per connection and a line cap

**Closes:** R14-F3.

**Change.** Per §2.3: accepting spawns a thread per connection, up to
16 at once, and the reader is bounded with `take` and `read_until` at
1 MiB. R14's sketch `4ac7e2d` is the reference.

**Gate.** Two tests in `transport.rs`.

- "an_idle_client_does_not_block_a_second": A connects and sends
  nothing, B sends one line. It prints
  `answered in X ms / bound 1000 ms`.
- "a_line_without_newline_is_capped": it sends 2 MiB with no newline
  and asserts the connection closed. A counting global allocator in
  the test binary asserts the growth is under 2 MiB, printing
  `grew X KiB / bound 2048 KiB`. The transport runs in-process now, so
  the test can count.

Accept direction: after both, a well-formed request on a fresh
connection is answered, and a 17th concurrent connection gets its
`-32000` line while the other 16 still answer. T39.0's winit idle and
flood cases go green.

**Injection.** (1) Serve connections inline instead of spawning: the
idle test goes red at the 1 s bound. (2) Replace the bounded read with
`lines()`: the growth test goes red by roughly the flood size.

**Docs in the same commit.** The results doc; the thread-rule
paragraph of the `agent` module doc, since connections now run in
parallel while requests are still answered on the loop; and the skill,
since one window now serves many clients.

**Effort:** S. **Depends on:** T39.1.

## T39.4 — the envelope is validated, and coordinates must be finite

**Closes:** R14-F7.

**Change.** Per §2.4. One validation function in the agent crate runs
at the top of `Agent::dispatch` and `Agent::dispatch_input`. It
requires an object with `jsonrpc == "2.0"`, a string `method`, an `id`
member, and `params` either absent or an object; anything else gets
`-32600`. One finite-number reader replaces the four `as f32` casts of
§1.4, and refuses a value that is not finite or whose magnitude is over
1e7 with `-32602`. Parse-error replies in the transport and in
`Session::agent` are built with `json!`, not `format!` into a raw
string, so serde's message text is escaped once.

**Gate.** T39.0's "the_envelope_is_validated" goes green, row by row,
with the expected code printed beside each. The accept row, a
well-formed `input.click`, still gets a result. Every existing agent
test, all of which already send `jsonrpc: "2.0"` and an `id`, stays
green: that is the wider accept direction, and it was checked while
writing this plan.

**Injection.** Restore `unwrap_or("")` and drop the validation: the
table goes red with `-32601` where `-32600` was expected. Remove the
finite check: the `x: 1e300` row returns a result.

**Docs in the same commit.** The protocol paragraph of the agent
crate's module doc; the skill; and `docs/plan-agent-parity.md` if it
states conformance.

**Effort:** S. **Depends on:** T39.1, because the transport's framing
refusal and this validation must agree on which code a batch gets.

## T39.5 — the agent's own panic is an error reply

**Closes:** R14-F5.

**Change.** Per §2.5. Wrap `Agent::dispatch_input` in `Session::agent`
in `catch_unwind` under `panic = "unwind"`, and reply `-32603` with
the method and the hook's location. `Live::pump` marks application
code, and the outer catch re-raises a marked panic.

**Gate.** T39.0's `fulgurite-shell` test goes green: the reply is
`-32603` naming the location, and the next `ui.getTree` is answered.
It prints `reply code / next call ok`. The **accept direction** has two
parts. An ordinary call before and after returns a result. And under
`Recover::Off`, an application panic raised through `input.click`
**still unwinds out of the session**; the test asserts it with its own
`catch_unwind` at the top.

**Injection.** (1) Remove the catch: the test binary dies with the
panic. (2) Remove the re-raise: the `Recover::Off` case goes red,
because the application's panic has turned into a reply.

**Docs in the same commit.** `docs/plan-panic-recovery.md` gets a note
that the agent's own panics are now caught and the application's are
not. Also the rustdoc on `Session::agent`, and the `hostile.rs`
header.

**Effort:** S. **Depends on:** nothing in this milestone. It can land
any time after T39.0.

## T39.6 — a descendant error names the selector once

**Closes:** R14-F6.

**Change.** `resolve_one` refuses an empty head or tail with `-32602`
"descendant step `>` needs a selector on both sides", before
recursing. The recursive failure is reported with a message built from
the full selector, and the `replace` is removed entirely (§1.6 says
why guarding the empty case is not enough).

**Gate.** T39.0's test goes green on all four cases, printing the reply
and request sizes. Accept direction: `#outer > #inner` still resolves,
and a genuinely missing inner id still reports its nearest misses.

**Injection.** Restore `m.replace(tail.trim(), selector)` alongside the
empty-side check: `#x > e` goes red on the "exactly once" assertion,
even though the three empty cases stay green.

**Docs in the same commit.** The selector grammar block at the top of
`crates/fulgurite-agent/src/lib.rs` (lines 10–17) states that both
sides of `>` are required.

**Effort:** S. **Depends on:** nothing. It touches no transport code
and can land at any point in the milestone.

## T39.7 — every client, against the finished transport

**Closes:** nothing new. This is rule 3's accept direction for the
milestone as a whole, recorded rather than assumed.

Run each existing client against the transport with T39.1–T39.6 in
place, and record pass or fail per client in the results doc:

- `scripts/agent_client.py`, through `just live-gate` and
  `scripts/ui_check.py`;
- the drive harness (`scripts/drive/harness.py` and
  `scripts/drive/run.py`) through `just widget-drive`, under both
  invalidation strategies;
- `just tasks-drive` (`scripts/tasks_drive.py`);
- `just hot-window-drive` (`scripts/hot_window_drive.py`);
- `just panic-drive` (`scripts/panic_drive.py` and the inline client in
  `scripts/panic_drive.sh`);
- `just agent-drive` (the inline client in `scripts/agent_drive.sh`).

**Gate.** Every client exits 0, and the results doc's table has one row
per client. **Injection.** Make the transport close after the first
reply on every connection. At least `just agent-drive` and
`just widget-drive` must fail, because both hold one connection for
many calls. This shows the sweep can see a transport that breaks
clients, and is not just a list of green runs.

**Effort:** S. **Depends on:** T39.1–T39.6.

## 4. Boundaries of this plan

- **A per-session token is deferred, not rejected.** It is the only
  option that separates users on a shared host. It would cost every
  client a file read and the skill a paragraph. Reopen it when either
  of these becomes true: the agent is documented for a multi-user host,
  or a method is added that writes a file, starts a build or loads a
  library. Then the cost of a same-host peer is no longer bounded by UI
  control. M37 added none of these (§1.1).
- **Same-user local processes are out of scope** (§2.1).
- **Method-level truthfulness is M48's**: `ui.waitSettled`, `app.perf`,
  the frame counters, `app.quit`, and the environment variable that
  names the address. `app.quit` in particular **waits for T39.1**: a
  method that ends the process, reachable from a web page, is a denial
  of service until the transport refuses the page.
- **R14-F4 and R14-F8** (the cost of `frame_done`) are also M48's, by
  assignment, although R14's review found them alongside these.
- **No finding here belongs elsewhere.** R14-F6 is a protocol-truth
  defect more than a transport one. It stays here because it is an
  amplification that T39.3's line cap bounds only on the request side,
  not on the reply side.

## 5. Kill rules and risks

There is no performance claim in this milestone, so there is no kill
rule. What would stop it, and what would reveal each risk:

- **A client that relies on a malformed line being skipped.** None was
  found by reading all eight. T39.7 is what would reveal one: a client
  red after T39.1 that was green before.
- **A browser that can put a JSON object on the first line.** The
  argument in §2.1 says none can. The captured fixtures show it for
  four real browser APIs. A new API that could (for example, a raw
  socket API exposed to pages) would appear as a new captured vector
  executing in `transport.rs`. That is why the fixtures are captured,
  not typed.
- **The winit event-loop test cannot run without a display.** It is
  `harness = false` and needs X11 or Wayland. Where there is no
  display, the display-free `transport.rs` carries the refusal, and the
  winit test is an assembly check only. Its skip prints a line; it
  never passes silently. It follows M43's skip policy (M43 §2.2): a
  plain `cargo test` may skip, but a recipe that names the display
  requires it, and there a missing display is a failure.
- **Thread-per-connection ordering.** Two clients' requests interleave
  on the loop. This is safe by the thread rule, but a harness that ran
  two clients against one window and compared frame numbers could see
  the other client's frames. No harness does this today. M48/T48.1
  documents what a reply's frame means.

## 6. What building found (2026-09-25)

Recorded as found, rather than folded into the sections above.

- **T39.0, the fixtures.** A real headless browser *was* available, so
  every "captured" fixture is captured, from two: **Firefox 156.0**
  (`/usr/lib/firefox/firefox --headless`) and **Chrome for Testing
  145.0.7632.6** (`chrome-headless-shell`). `scripts/capture_browser_requests.py`
  serves a page on one loopback port and records what the browser sends
  to another, so the page is cross-origin to the endpoint as a page the
  developer happens to have open is. Eight captured files (four vectors
  × two browsers) and the two hand-written ones live in
  `crates/fulgurite-agent/tests/browser/`, named by browser and
  version, since a header comment inside a byte-exact file would stop
  it being byte-exact. Each body carries two JSON-RPC lines, a click on
  counter-window's `#inc` and one on tasks-demo's `#spawn-one`, so one
  fixture serves both live steps. Red at the base (`b213ae0`): **16
  methods executed** across the eight body-carrying vectors, two each;
  the two WebSocket upgrades executed nothing but left the connection
  open for the next line.
- **T39.0, the replay helper.** `play` in the winit test first
  half-closed its socket after writing, which hands the server an EOF
  and made *every* loop, the broken one included, "close the
  connection". A browser keeps its write side open while it waits for
  a response, so the gate does too. That is the "setup silently did not
  happen" shape: the closed-connection half of the gate could not fail.
- **T39.0, the descendant gate.** §T39.0 item 6 names `#x >`, `> #x`,
  `#x >   ` and `#x > e`. Against `descendant.rs`'s fixture `#x` does
  not exist, so the *head's* error returns before the tail is looked at
  and no case reaches the `replace`; four of the plan's cases were red
  for a different reason than the finding. The gate uses `#first`, an
  owner that resolves, and every case then shows the finding: `#first >`
  draws a 134 B message against a 96 B bound with the selector in it 15
  times, and `#first > e` puts the selector in place of every `e`.
- **T39.0, the live step.** `scripts/live_gate.sh` ran counter-window
  through `cargo run`, so `$!` was cargo's pid and the port-ownership
  check could never pass. It runs the built binary now. Red:
  `#value 0 -> 1` (the fixture has one `#inc` line; the plan's
  `0 -> 10` assumed ten).

- **T39.1, the injections.** §T39.1 predicted `methods executed: 1 /
  bound 0` under the first injection. It is **2** on every body-carrying
  vector, because each fixture carries two JSON lines (one per live
  application). Injections 2 and 3 stayed green, as predicted. The
  static check's injection, counter-window's loop restored, names
  `examples/counter-window/src/agent_socket.rs:123`. The live
  `tasks_drive.py` step was first vacuous in the other direction: it read
  `#counts` the moment the replay's connection closed, and a spawned
  job's result lands ~40 ms later, so a replay that *had* executed would
  have read as one that had not. `scripts/browser_replay.py` now waits
  500 ms, and the first injection turns both live steps red
  (`one=0 -> one=1`, `#value 0 -> 1`).
- **T39.3, the flood size.** §T39.3 sends 2 MiB. Under the old
  `lines()` a 2 MiB line grows its buffer to exactly 2 MiB (capacity
  doubles through powers of two), which sits *on* the 2 MiB bound rather
  than over it, so the red would have been decided by a few bytes of
  unrelated allocation. The gate sends 8 MiB: red under the unbounded
  read is 8264 KiB, green is 1207 KiB. And the growth is counted by a
  global allocator in `transport.rs`, whose other tests run in parallel;
  they allocate kilobytes against a bound of megabytes.
- **T39.3, a refusal has to arrive.** The 17th-connection gate failed
  four runs in six after T39.3 landed: the server wrote `-32000` and
  dropped the socket while the client's request was still unread, the
  kernel answered that with a reset, and Linux discards received-but-
  unread data on a reset, so the client read an error, not the reply.
  Every refusal now half-closes and reads what the peer is still
  sending, for at most 250 ms and 64 KiB, before the socket drops; the
  over-limit one does it off the accept thread. 8 runs of 8 green after.
- **T39.5, the marker.** §2.5 says `Live::pump` "marks the call as
  application code". It is a payload wrapper rather than a flag: the
  pump catches whatever escapes it and resumes the unwind with the
  payload boxed in a private `AppPanic`, and `Session::agent` re-raises
  the inner payload. A flag set for the call's duration would have to be
  cleared on the unwinding path, which is exactly where a drop guard
  would clear it too early. Its cost: a caller that drove `Live` without
  `Session::agent` would see the wrapper — the first injection showed it.
- **T39.7, the injection.** The plan required `agent-drive` and
  `widget-drive` to fail under close-after-first-reply; **all six**
  clients failed, since each makes more than one call per connection.
- **T39.1, AGENT.md's queue entry** is not in this commit: the
  implementation brief reserves AGENT.md for the coordinator's merge.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R14-F1 | S0 | CLOSED 2026-09-25 | T39.1 |
| R14-F2 | S2 | CLOSED 2026-09-25 | T39.2 |
| R14-F3 | S2 | CLOSED 2026-09-25 | T39.3 |
| R14-F5 | S3 | CLOSED 2026-09-25 | T39.5 |
| R14-F6 | S3 | CLOSED 2026-09-25 | T39.6 |
| R14-F7 | S3 | CLOSED 2026-09-25 | T39.4 |
