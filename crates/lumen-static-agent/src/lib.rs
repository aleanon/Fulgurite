//! `lumen-static-agent` — the JSON-RPC 2.0 agent protocol (parity plan
//! P0/P1, `docs/plan-agent-parity.md`).
//!
//! Mirrors `lumen-agent`'s envelope and method vocabulary where the
//! semantics coincide, so agent tooling transfers. Dispatch is
//! in-process; the socket every live endpoint serves it on is
//! [`transport`] (M39), newline-delimited JSON over TCP, which ends a
//! connection on any line that is not a JSON object. Queries run over an
//! already-laid-out tree, and `input.*` goes through the host's one
//! queue.
//!
//! **The envelope is validated at dispatch** (M39/T39.4): a request is
//! an object with `jsonrpc: "2.0"`, an `id` (string, number or null), a
//! string `method`, and `params` absent or an object — anything else is
//! `-32600`, so a batch and a notification are refused rather than run.
//! A coordinate must be finite and within ±1e7, or it is `-32602`.
//!
//! Selector grammar (space-separated terms, all must match):
//! - `#save`        author id (`#[widget_id("save")]`)
//! - `label`        widget kind name
//! - `~text`        the text it shows contains "text" (value *or* label)
//! - `#a > #b`     the `#b` **inside** `#a`; a part id need only be
//!   unique within its owner (M27/T27.0). **Both sides are required**:
//!   `#a >` and `> #b` are `-32602` (M39/T39.6)
//! - `@app.rs:42`   source provenance (file suffix, optional line)
//! - `7`            table index
//!
//! `ui.explain` answers the question those selectors are usually asked
//! in service of — "I clicked it and nothing happened" — by running the
//! real hit test rather than by describing the tree.
//!
//! Resolution errors teach (Lumen's ergonomic, adopted): not-found
//! carries the nearest misses, ambiguity carries the candidates.
//! Selectors resolve over main-table nodes; repeater instances are
//! reachable through `ui.probe`/`ui.getTree` (documented P1 limit).
#![forbid(unsafe_code)]

use lumen_static::desc::{NodeDesc, NodeState};
use lumen_static::event::{Event, Key};
use lumen_static::hit::Hit;
use lumen_static::observe::{self, accesskit};
use lumen_static::widget::Node;
use lumen_static::{pass, Widget};
use serde_json::{json, Map, Value};

mod audit;
/// M12/T12.1: the locator and expectation API application tests use.
pub mod harness;
mod journal;
pub mod png;
pub mod session;
pub mod trace;
/// M39: the one socket every agent endpoint serves through.
pub mod transport;

use audit::Auditor;
use journal::{Coalesced, Journal};

/// A laid-out tree, borrowed for one dispatch.
///
/// The widget itself plus its structural table and layout output — no
/// base pointer and no kind list, since traversal recurses
/// (`docs/results-traversal.md`).
#[derive(Clone, Copy)]
pub struct TreeRef<'a> {
    pub root: &'a dyn Node,
    pub table: &'a [NodeDesc],
    pub states: &'a [NodeState],
    pub styles: &'a [lumen_static::Style],
    /// The stylesheet in force and what the sheet lint found in it —
    /// M25/T25.2. Empty unless a host supplies them with
    /// [`TreeRef::with_sheet`]; `ui.lint` reports the findings as
    /// `W1010`.
    pub sheet: &'a [lumen_static::Rule],
    pub sheet_findings: &'a [lumen_static::style::SheetFinding],
    /// One [`Seed`](lumen_static::style::Seed) per table index — the
    /// kind and defaults of the node that *belongs* at each index,
    /// M26/T26.2.
    ///
    /// **Reporting reads kinds from here, never from the instance.** A
    /// node inside an inactive enum variant has no live instance, so
    /// `Node::node(i)` answers with the container standing in for it and
    /// `kind_name` then reports the container's kind for every hidden
    /// node. That is the defect M24/T24.1 fixed in the cascade, and this
    /// is the same fix in the surface next door. `live_nodes` is the one
    /// exception and says why at its own definition.
    ///
    /// Same length as `table`, by construction: both come from `W` in
    /// [`TreeRef::of_root`].
    pub seeds: &'a [lumen_static::style::Seed],
}

/// Which indices `Node::node` actually answers *for*, into `out`.
///
/// `Node::node(i)` answers with the nearest enclosing **container** when
/// `i` falls inside a branch that is not currently laid out — an
/// `Option::None`, an inactive enum variant, a closed `Popup`. That is
/// deliberate: the agent still walks those indices and reports them, so
/// a tree dump names every node whichever variant is showing.
///
/// **But that stand-in must never be painted or probed at `i`.** It lays
/// its own children out from `i` rather than from its own index, so the
/// deeper the hidden node the further past the end of `states` it
/// writes — a release-build panic the first time a real tree put an enum
/// switch near the end of the table (M24/T24.3). Earlier trees escaped
/// by luck: no hidden subtree, or one near enough the start that the
/// overrun stayed in bounds and quietly hashed the wrong node.
///
/// # Why not "zero area"
///
/// Tried first, and wrong. A hidden branch *is* zeroed by `layout`, but
/// so is a leaf that has content and was given no room — which is
/// exactly `W1001`, the lint this crate exists to make possible. Both
/// look identical from the state array, and gating on area turned the
/// invisible-content lint off.
///
/// # What it actually tests
///
/// A node is a stand-in iff `node(i)` is **the same object** as
/// `node(parent(i))` — same address *and* same kind. A live node is
/// always a distinct object from its parent's.
///
/// The kind is checked as well as the address because a zero-sized first
/// field shares its parent's address: `struct Row { s: Spacer }` puts
/// `&row.s` and `&row` at the same place. Their kinds differ, so they
/// are told apart — and dropping the kind from this comparison is shown
/// to break `changes.rs`, so it is load-bearing rather than defensive.
/// The residue it cannot see is a wrapper both at offset zero and of the
/// same kind as its own child, which needs a type to contain itself.
///
/// One `node()` call per index, walking the table in order, so this is
/// O(n) rather than the O(n·depth) an ancestor walk would cost.
pub fn live_nodes(root: &dyn Node, table: &[NodeDesc], out: &mut Vec<bool>) {
    // **The one place that must keep asking the instance** — M26/T26.2.
    // Everything that *reports* a kind reads `TreeRef::seeds`, because a
    // hidden node's instance is its container standing in. Here the
    // stand-in is precisely what is being detected: a hidden node and the
    // container it stands in for share an address *and* a kind, and the
    // seed kinds differ per index, so a seed-built identity could never
    // match and nothing would be reported hidden.
    fn ident(n: &dyn Node) -> (*const (), &'static str) {
        (n as *const dyn Node as *const (), n.kind_name())
    }
    out.clear();
    out.resize(table.len(), true);
    let mut id: Vec<(*const (), &'static str)> = Vec::with_capacity(table.len());
    for i in 0..table.len() {
        id.push(ident(root.node(i)));
    }
    for i in 0..table.len() {
        let p = table[i].parent;
        if p == lumen_static::NO_PARENT {
            continue;
        }
        // A stand-in *and* everything under it: the parent's liveness
        // carries down, so one pass in table order (DFS pre-order, so
        // parents first) settles the whole subtree.
        out[i] = out[p as usize] && id[i] != id[p as usize];
    }
}

impl<'a> TreeRef<'a> {
    pub fn of_root<W: Widget>(
        root: &'a W,
        styles: &'a [lumen_static::Style],
        states: &'a [NodeState],
    ) -> Self {
        TreeRef {
            root,
            table: W::SUB_TABLE,
            states,
            styles,
            sheet: &[],
            sheet_findings: &[],
            seeds: W::SEEDS,
        }
    }

    /// The same view, carrying the stylesheet in force and its lint
    /// findings (M25/T25.2) — what `lumen_static_shell::Host::stylesheet`
    /// and `Host::sheet_findings` return.
    pub fn with_sheet(
        mut self,
        sheet: &'a [lumen_static::Rule],
        findings: &'a [lumen_static::style::SheetFinding],
    ) -> Self {
        self.sheet = sheet;
        self.sheet_findings = findings;
        self
    }
}

/// The same view at a frame boundary, where the host owns the tree
/// mutably. The audit's intrinsic probe needs it: measuring what a node
/// *wants* to be means calling `layout`, which takes `&mut self`.
pub struct TreeMut<'a> {
    pub root: &'a mut dyn Node,
    pub table: &'a [NodeDesc],
    pub states: &'a [NodeState],
    pub styles: &'a [lumen_static::Style],
    /// As [`TreeRef::seeds`] — M26/T26.2.
    pub seeds: &'a [lumen_static::style::Seed],
}

impl<'a> TreeMut<'a> {
    pub fn of_root<W: Widget>(
        root: &'a mut W,
        styles: &'a [lumen_static::Style],
        states: &'a [NodeState],
    ) -> Self {
        TreeMut { root, table: W::SUB_TABLE, states, styles, seeds: W::SEEDS }
    }

    pub fn as_ref(&self) -> TreeRef<'_> {
        TreeRef {
            root: &*self.root,
            table: self.table,
            states: self.states,
            styles: self.styles,
            sheet: &[],
            sheet_findings: &[],
            seeds: self.seeds,
        }
    }
}

/// Per-session agent state: frame counter, frame timings, and the
/// change journal (P2), all fed by the host at each frame boundary.
pub struct Agent {
    pub frame: u64,
    /// The last `PERF_RING` frames' timings, oldest overwritten. Sized
    /// once in `new`; `frame_done` never grows it.
    perf: Vec<FrameTiming>,
    perf_next: usize,
    journal: Journal,
    auditor: Auditor,
}

/// Frames `app.perf` summarises. Two seconds at 60 Hz: enough for a
/// p95 that means something, small enough to copy and sort per query.
const PERF_RING: usize = 128;

/// Where one frame's time went, as the host measured it. All in
/// nanoseconds; **a host that did not measure a stage reports `None`**,
/// which `app.perf` answers as `null` — never `0`, which a budget check
/// passes (M48/T48.3, R13-F2: every turnkey window reported zeros for
/// every stage, and `scripts/ui_check.py`'s budgets passed on them).
/// `input_latency_ns` is `None` for a frame that consumed no input.
///
/// `input_latency_ns` is the number a person feels: from the oldest
/// input this frame consumed arriving at the shell to the frame that
/// shows its effect being presented. Layout and paint are microseconds
/// here; the first live tally still took most of a second to react,
/// and no field the agent then had could see why
/// (`docs/results-frame-cost.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameTiming {
    pub layout_ns: Option<u64>,
    pub paint_ns: Option<u64>,
    /// Display list to pixels.
    pub render_ns: Option<u64>,
    /// Pixels to the surface.
    pub present_ns: Option<u64>,
    /// The whole redraw, input drain to presented.
    pub frame_ns: Option<u64>,
    pub input_latency_ns: Option<u64>,
    /// Pixels the frame actually redrew — the area of the damage
    /// regions, or the viewport when everything was redrawn, in
    /// **physical** pixels (M40/T40.1; before it, logical rects counted
    /// as physical, short by the scale squared). Not comparable with
    /// `ui.lastDamage`, which is the journal's, in logical pixels.
    /// `None` from a host that renders whole frames and does not track
    /// damage.
    ///
    /// Here because "why is this frame slow" is answered by *how much*
    /// was drawn far more often than by which stage drew it, and a
    /// partial-redraw path that has quietly degraded to full frames is
    /// otherwise invisible: every stage still looks reasonable.
    pub damaged_px: Option<u64>,
}

impl FrameTiming {
    /// Layout and paint only — what a headless host can measure.
    pub fn passes(layout_ns: u64, paint_ns: u64) -> Self {
        FrameTiming { layout_ns: Some(layout_ns), paint_ns: Some(paint_ns), ..Default::default() }
    }

    fn json(&self) -> Value {
        json!({
            "layout_ns": self.layout_ns,
            "paint_ns": self.paint_ns,
            "render_ns": self.render_ns,
            "present_ns": self.present_ns,
            "frame_ns": self.frame_ns,
            "input_latency_ns": self.input_latency_ns,
            "damaged_px": self.damaged_px,
        })
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

const NOT_FOUND: i64 = -32001;
const AMBIGUOUS: i64 = -32002;
const INVALID_PARAMS: i64 = -32602;

/// Longest string `input.type` will turn into queued events in one call.
///
/// Generous for anything a person would type or paste, and small enough
/// that the queue cannot be used to exhaust the host's memory. See the
/// method for why this exists.
const MAX_TYPE_CHARS: usize = 4096;

/// How many events an input method queues before it pumps — M44/T44.5.
///
/// Half the default queue (`InputQueue::<64>::CAP`), so a burst leaves
/// room for whatever the host had queued already. `input.type` and
/// `input.fill` used to queue every character and pump once, into a
/// 64-slot queue: 100 characters left 64 in the field and a reply saying
/// 100. `Host::paste` chunks the same way for the same reason.
const INPUT_CHUNK: usize = lumen_static::InputQueue::<64>::CAP / 2;

/// Queue `events`, pumping every [`INPUT_CHUNK`] and once at the end.
/// Returns the last frame's number and whether anything took an event.
fn feed(host: &mut dyn AgentHost, events: impl IntoIterator<Item = Event>) -> (u64, bool) {
    let mut handled = false;
    let mut queued = 0;
    for ev in events {
        if queued == INPUT_CHUNK {
            handled |= host.pump().1;
            queued = 0;
        }
        host.enqueue(ev);
        queued += 1;
    }
    let (frame, h) = host.pump();
    (frame, handled | h)
}

/// How many inputs the host lost between two readings of its drop
/// count, or `None` from a host that cannot say.
fn lost(before: Option<u32>, after: Option<u32>) -> Option<u32> {
    Some(after?.saturating_sub(before?))
}
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_REQUEST: i64 = -32600;

/// Largest coordinate magnitude a method accepts (M39/T39.4). Far past
/// any window, and small enough that an `f32` still resolves a pixel.
const MAX_COORD: f64 = 1e7;

/// **The envelope, validated** — M39/T39.4 (R14-F7). The request's `id`
/// and `method`, or the `-32600` reply that refuses it.
///
/// One function, called first by both [`Agent::dispatch`] and
/// [`Agent::dispatch_input`], so an in-process caller (`TestApp`, a
/// replayed recording) gets the same answer as the socket. Until M39 the
/// method was read as `as_str().unwrap_or("")`, so a batch, a bare number
/// or a non-string method fell through to "unknown method ``", a request
/// with no `id` was executed and answered, and `jsonrpc: "1.0"` was
/// accepted.
///
/// **A request with no `id` is refused, not run silently.** JSON-RPC
/// says a notification gets no reply; a write method that runs and says
/// nothing is the worse surprise, and no client sends one. A batch is
/// refused because its `input.*` steps would run with no frame between
/// them (`docs/plan-agent-transport.md` §2.4).
fn envelope(req: &Value) -> Result<(Value, &str), Value> {
    let refuse = |id: &Value, message: &str| {
        json!({ "jsonrpc": "2.0", "id": id, "error": {
            "code": INVALID_REQUEST, "message": message, "data": Value::Null } })
    };
    let Some(obj) = req.as_object() else {
        return Err(refuse(
            &Value::Null,
            if req.is_array() {
                "batches are not supported: send one request per line"
            } else {
                "a request is a JSON object"
            },
        ));
    };
    // An id of the wrong type is not echoed: it is not an id.
    let id = match obj.get("id") {
        Some(v @ (Value::String(_) | Value::Number(_) | Value::Null)) => Some(v.clone()),
        Some(_) => return Err(refuse(&Value::Null, "`id` must be a string, a number or null")),
        None => None,
    };
    let shown = id.clone().unwrap_or(Value::Null);
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(refuse(&shown, "`jsonrpc` must be \"2.0\""));
    }
    let Some(id) = id else {
        return Err(refuse(&shown, "a request needs an `id`: notifications are not supported"));
    };
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return Err(refuse(&id, "`method` must be a string"));
    };
    if !matches!(obj.get("params"), None | Some(Value::Object(_))) {
        return Err(refuse(&id, "`params` must be an object when present"));
    }
    Ok((id, method))
}

/// A coordinate, **finite and within [`MAX_COORD`]** — M39/T39.4. Four
/// sites cast `as f32` with no check, so `x: 1e300` was a success
/// answered `at: [null, null]`.
fn coord(v: f64, key: &str) -> Result<f32, (i64, String, Value)> {
    if v.is_finite() && v.abs() <= MAX_COORD {
        Ok(v as f32)
    } else {
        Err((INVALID_PARAMS, format!("`{key}` must be a finite number within ±1e7, got {v}"), Value::Null))
    }
}

type RpcResult = Result<Value, (i64, String, Value)>;

impl Agent {
    pub fn new() -> Self {
        Agent {
            frame: 0,
            perf: Vec::with_capacity(PERF_RING),
            perf_next: 0,
            journal: Journal::default(),
            auditor: Auditor::default(),
        }
    }

    /// Measure text the way the host does. The ambient audit's
    /// intrinsic-size probe uses this; left at the block stand-in, a
    /// host laying out with a real face gets truncation findings for
    /// labels that fit.
    pub fn set_text_measure(&mut self, measure: Box<dyn lumen_static::TextMeasure>) {
        self.auditor.measure = measure;
    }

    /// The most recent frame's timings.
    pub fn last_timing(&self) -> FrameTiming {
        if self.perf.is_empty() {
            return FrameTiming::default();
        }
        self.perf[(self.perf_next + self.perf.len() - 1) % self.perf.len()]
    }

    /// **The frame an `input.*` reply names** — M48/T48.1 (R13-F3,
    /// `docs/plan-agent-truth.md` §2.1): the first journal frame that
    /// includes the input, `self.frame + 1`, because the journal records
    /// the frame the host draws after the pump the reply ran.
    ///
    /// It was the host's own counter, which moves twice per input in a
    /// window — once for the pump inside the RPC and once for the redraw
    /// after it — while this one moves once per `frame_done`. The two
    /// drifted apart on every click after the first, and
    /// `since = reply.frame - 1` found nothing (`tests/agent_frames.rs`
    /// in the shell). Only this counter means anything to `since`; the
    /// host's number stays in the reply as `host_frame`, for debugging.
    /// Two inputs answered before one redraw both name the same frame,
    /// correctly: both land in it.
    pub fn reply_frame(&self) -> u64 {
        self.frame + 1
    }

    /// `app.perf`: the last frame, and p50/p95/max over the ring — the
    /// summary a latency budget is checked against.
    fn perf(&self, params: &Value) -> Value {
        // An empty window is `null`, not 0: a stage nothing measured
        // must not read as a stage that took no time (M48/T48.3).
        fn pct(sorted: &[u64], p: f64) -> Option<u64> {
            if sorted.is_empty() {
                return None;
            }
            Some(sorted[((sorted.len() - 1) as f64 * p).round() as usize])
        }
        type Get = fn(&FrameTiming) -> Option<u64>;
        let mut summary = Map::new();
        let fields: [(&str, Get); 7] = [
            ("layout_ns", |t| t.layout_ns),
            ("paint_ns", |t| t.paint_ns),
            ("render_ns", |t| t.render_ns),
            ("present_ns", |t| t.present_ns),
            ("frame_ns", |t| t.frame_ns),
            ("input_latency_ns", |t| t.input_latency_ns),
            ("damaged_px", |t| t.damaged_px),
        ];
        for (name, get) in fields {
            let mut v: Vec<u64> = self.perf.iter().filter_map(get).collect();
            v.sort_unstable();
            summary.insert(
                name.into(),
                json!({
                    "p50": pct(&v, 0.5),
                    "p95": pct(&v, 0.95),
                    "max": v.last().copied(),
                    "n": v.len(),
                }),
            );
        }
        let mut out = json!({
            "frame": self.frame,
            "last": self.last_timing().json(),
            "window": summary,
            "samples": self.perf.len(),
        });
        // `since`: what the surface presented over the journal frames
        // after it — M48/T48.4. The drive asks this of an action's frames
        // and prints it beside the journal's own (logical) damage, which
        // is a different quantity (M40 §2.1).
        if let Some(since) = params.get("since").and_then(Value::as_u64) {
            let n = self.frame.saturating_sub(since).min(self.perf.len() as u64) as usize;
            let len = self.perf.len();
            let (mut px, mut measured) = (0u64, 0usize);
            for k in 0..n {
                let t = &self.perf[(self.perf_next + len - 1 - k) % len];
                if let Some(d) = t.damaged_px {
                    px += d;
                    measured += 1;
                }
            }
            out["since"] = json!({
                "since": since,
                "frames": n,
                "truncated": self.frame.saturating_sub(since) > n as u64,
                "damaged_px": if measured > 0 { json!(px) } else { Value::Null },
            });
        }
        out
    }

    /// Record a completed frame: the host calls this after layout +
    /// paint, and it is what makes `ui.lastChange`/`ui.lastDamage`
    /// answerable.
    ///
    /// Runs both the journal's diff and the ambient audit — the audit is
    /// the larger half, and this doc omitted it entirely until
    /// 2026-09-02. Measured at 901 nodes: 46 µs against a 7.3 µs frame
    /// with few findings, 54 µs with 132, and **zero allocations in
    /// steady state** either way (`docs/results-agent-cost.md`). A dev
    /// surface, not a release path.
    pub fn frame_done(&mut self, tree: &mut TreeMut, timing: FrameTiming) {
        self.frame += 1;
        if self.perf.len() < PERF_RING {
            self.perf.push(timing);
        } else {
            self.perf[self.perf_next] = timing;
        }
        self.perf_next = (self.perf_next + 1) % PERF_RING;
        let structural_reset = self.journal.record(self.frame, &tree.as_ref());
        self.auditor.record(self.frame, tree, structural_reset);
    }

    /// Dispatch one JSON-RPC 2.0 request, returning the response object.
    pub fn dispatch(&self, tree: &TreeRef, req: &Value) -> Value {
        let (id, method) = match envelope(req) {
            Ok(ok) => ok,
            Err(refused) => return refused,
        };
        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

        match self.handle(tree, method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message, data)) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": code, "message": message, "data": data }
            }),
        }
    }

    /// Dispatch including the `input.*` methods, which need somewhere to
    /// put events. Everything the agent synthesizes goes through the
    /// host's **one queue** — never straight into a handler — so an agent
    /// session replays as a test (plan E4).
    pub fn dispatch_input(&self, host: &mut dyn AgentHost, req: &Value) -> Value {
        let (id, method) = match envelope(req) {
            Ok(ok) => ok,
            Err(refused) => return refused,
        };
        // M36: a panic is host state, so these answer from the host.
        if method == "app.failure" {
            return json!({ "jsonrpc": "2.0", "id": id, "result": host.failure() });
        }
        if method == "app.retry" {
            let retried = host.retry();
            host.pump();
            return json!({ "jsonrpc": "2.0", "id": id, "result": { "retried": retried } });
        }
        // **While a panic is shown, input is refused** (M36 §2.2): pumping
        // would be an automatic retry.
        if method.starts_with("input.") && !host.failure()["failure"].is_null() {
            return json!({ "jsonrpc": "2.0", "id": id, "error": {
                "code": INVALID_PARAMS,
                "message": "the application panicked — read app.failure, then call app.retry",
                "data": Value::Null,
            }});
        }
        // M48/T48.2: settling is host state — the last frame's animation,
        // the inbox, jobs in flight, the next timer.
        if method == "ui.waitSettled" {
            if let Some(mut result) = host.settled() {
                result["frame"] = self.frame.into();
                return json!({ "jsonrpc": "2.0", "id": id, "result": result });
            }
        }
        if method == "app.shadow" {
            let result = host.shadow().unwrap_or_else(|| json!({ "supported": false }));
            return json!({ "jsonrpc": "2.0", "id": id, "result": result });
        }
        // M42/T42.1: the tree answers the diagnostics; the host adds which
        // process it is, so a drive can check it drove the build it
        // launched rather than whatever held the port.
        if method == "app.diagnostics" {
            let mut reply = {
                let tree = host.tree();
                self.dispatch(&tree, req)
            };
            if let Some(result) = reply.get_mut("result") {
                result["process"] = host.process_info();
                // M44/T44.5: input the host's queue lost for want of
                // room, ever; `null` from a host that cannot say.
                result["dropped_inputs"] = json!(host.dropped_inputs());
            }
            return reply;
        }
        if !method.starts_with("input.") && method != "ui.screenshot" && method != "app.setPalette"
        {
            let tree = host.tree();
            return self.dispatch(&tree, req);
        }
        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.handle_input(host, method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message, data)) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": code, "message": message, "data": data }
            }),
        }
    }

    fn handle_input(&self, host: &mut dyn AgentHost, method: &str, params: &Value) -> RpcResult {
        match method {
            // M25/T25.3: swap the palette a running window's looks are
            // resolved against. By name, because a palette is a theme an
            // agent chooses rather than seven colours it should have to
            // spell; `applied: false` from a host that cannot, so a caller
            // does not mistake "unsupported" for "done".
            "app.setPalette" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((
                    INVALID_PARAMS,
                    "missing `name`".into(),
                    json!({ "known": ["light", "dark"] }),
                ))?;
                let palette = match name {
                    "light" => lumen_static::style::Palette::LIGHT,
                    "dark" => lumen_static::style::Palette::DARK,
                    other => {
                        return Err((
                            INVALID_PARAMS,
                            format!("unknown palette `{other}`"),
                            json!({ "known": ["light", "dark"] }),
                        ))
                    }
                };
                let applied = host.set_palette(palette);
                let host_frame = host.pump().0;
                Ok(json!({ "applied": applied, "palette": name, "frame": self.reply_frame(), "host_frame": host_frame }))
            }
            // `clicks` (1 to 3, default 1) makes it a double or triple
            // click (M45/T45.6): one press and release per click, the
            // n-th press carrying `clicks: n`, all in one pump — what the
            // window's click counter produces for presses in quick
            // succession.
            "input.click" | "input.invokeAction" => {
                let (x, y) = self.point_param(host, params)?;
                let n = match params.get("clicks") {
                    None => 1,
                    Some(v) => match v.as_u64() {
                        Some(n @ 1..=3) => n as u8,
                        _ => {
                            return Err((INVALID_PARAMS, "`clicks` must be 1, 2 or 3".into(), v.clone()))
                        }
                    },
                };
                for k in 1..=n {
                    host.enqueue(Event::PointerDown { x, y, button: lumen_static::event::PointerButton::Left, clicks: k });
                    host.enqueue(Event::up(x, y));
                }
                let (f, h) = host.pump();
                Ok(json!({ "frame": self.reply_frame(), "host_frame": f, "handled": h, "at": [x, y] }))
            }
            // A press held open, and its release — the two halves of a
            // click, separately. Without them an agent cannot observe a
            // pressed control at all: `input.click` queues down and up
            // into the same pump, so the frame it returns has already
            // released. `ui_check.py press` is the caller.
            "input.pointerDown" => {
                let (x, y) = self.point_param(host, params)?;
                host.enqueue(Event::down(x, y));
                let (f, h) = host.pump();
                Ok(json!({ "frame": self.reply_frame(), "host_frame": f, "handled": h, "at": [x, y] }))
            }
            "input.pointerUp" => {
                let (x, y) = self.point_param(host, params)?;
                host.enqueue(Event::up(x, y));
                let (f, h) = host.pump();
                Ok(json!({ "frame": self.reply_frame(), "host_frame": f, "handled": h, "at": [x, y] }))
            }
            "input.hover" => {
                let (x, y) = self.point_param(host, params)?;
                host.enqueue(Event::PointerMove { x, y });
                let (f, h) = host.pump();
                Ok(json!({ "frame": self.reply_frame(), "host_frame": f, "handled": h, "at": [x, y] }))
            }
            "input.scroll" => {
                let (x, y) = self.point_param(host, params)?;
                let dy = match params.get("dy") {
                    None => 0.0,
                    Some(v) => coord(v.as_f64().unwrap_or(f64::NAN), "dy")?,
                };
                host.enqueue(Event::Scroll { x, y, dy });
                let (f, h) = host.pump();
                Ok(json!({ "frame": self.reply_frame(), "host_frame": f, "handled": h, "at": [x, y], "dy": dy }))
            }
            "input.key" => {
                let name = params.get("key").and_then(Value::as_str).ok_or((
                    INVALID_PARAMS,
                    "missing `key`".into(),
                    Value::Null,
                ))?;
                let key = parse_key(name).ok_or_else(|| {
                    (INVALID_PARAMS, format!("unknown key `{name}`"), json!({ "known": KEY_NAMES }))
                })?;
                let down = params.get("down").and_then(Value::as_bool).unwrap_or(true);
                // `mods`, a list of "shift", "ctrl", "alt" and "meta", are
                // held for this key and released after it (M45/T45.6), so
                // Ctrl+Left is one call rather than three.
                let mods = match params.get("mods") {
                    None => None,
                    Some(v) => Some(parse_mods(v)?),
                };
                if let Some(m) = mods {
                    host.enqueue(Event::Modifiers(m));
                }
                host.enqueue(Event::Key { key, down });
                if mods.is_some() {
                    host.enqueue(Event::Modifiers(lumen_static::event::Mods::NONE));
                }
                let (f, h) = host.pump();
                Ok(json!({ "frame": self.reply_frame(), "host_frame": f, "handled": h }))
            }
            // P5 of docs/plan-agent-parity.md, arriving with the first
            // host that renders. The image is a PNG so the agent can
            // open it without leaving the protocol.
            "ui.screenshot" => {
                // `source: "surface"` reads the window back from the
                // display server instead of returning the canvas — the
                // only way to see the blit and the present, which every
                // other method is downstream of.
                let surface = params.get("source").and_then(Value::as_str) == Some("surface");
                let (w, h, png) = if surface {
                    let (w, h, rgba) = host.presented().ok_or((
                        INVALID_PARAMS,
                        "this host cannot read its window back (softbuffer implements it on \
                         X11 and Web only) — treat that as not checkable, not as a match"
                            .into(),
                        Value::Null,
                    ))?;
                    (w, h, png::encode_rgba(w, h, &rgba))
                } else {
                    let (w, h, rgba) = host.frame().ok_or((
                        INVALID_PARAMS,
                        "this host does not render — nothing to screenshot".into(),
                        Value::Null,
                    ))?;
                    (w, h, png::encode_rgba(w, h, rgba))
                };
                Ok(json!({
                    "width": w,
                    "height": h,
                    "format": "png",
                    "source": if surface { "surface" } else { "canvas" },
                    // Physical pixels per logical pixel: the image is in
                    // physical units and `ui.getLayout` is in logical
                    // ones, so a client comparing them needs this.
                    "scale": host.scale(),
                    "image_base64": png::base64(&png),
                }))
            }
            // Replace a field's whole value in one call, through the
            // queue.
            //
            // `app.setValue` would be the obvious method and is
            // deliberately absent: it writes widget state behind the
            // queue's back, so a session using it would not replay,
            // which is the property the whole input surface exists for
            // (`docs/plan-agent-parity.md`). This does the same job with
            // real events — focus, End, one Backspace per existing
            // character, then the text — so the recording is still a
            // test. Measured before it existed: 23 RPCs to replace a
            // 20-character value, and the agent had to read the old
            // value first to know how many backspaces to send.
            "input.fill" => {
                let text = params.get("text").and_then(Value::as_str).ok_or((
                    INVALID_PARAMS,
                    "missing `text`".into(),
                    Value::Null,
                ))?;
                let n = text.chars().count();
                if n > MAX_TYPE_CHARS {
                    return Err((
                        INVALID_PARAMS,
                        format!("`text` is {n} characters; the limit is {MAX_TYPE_CHARS}"),
                        json!({ "chars": n, "limit": MAX_TYPE_CHARS }),
                    ));
                }
                let (x, y, existing) = {
                    let tree = host.tree();
                    let tree = &tree;
                    let ix = Index::build(tree);
                    let e = resolve_one(&ix, selector_param(params)?)?;
                    let i = e.idx.ok_or((
                        NOT_FOUND,
                        "selector resolved to virtualized content; fill it by coordinates and \
                         `input.type`"
                            .into(),
                        Value::Null,
                    ))?;
                    let st = &tree.states[i];
                    let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
                    // Refuse rather than type into whatever *is* there.
                    // A fill that misses its target does not merely fail:
                    // it sends a run of backspaces to whichever control
                    // holds focus, which is destructive and silent.
                    if !reaches(tree, i, x, y) {
                        return Err((
                            NOT_FOUND,
                            "that control is not where a click would land, so filling it would \
                             type into something else — call `input.scrollIntoView` first, or \
                             ask `ui.explain` why"
                                .into(),
                            json!({ "at": [x, y] }),
                        ));
                    }
                    let existing = e.node.value().map(|v| v.chars().count()).unwrap_or(0);
                    (x, y, existing)
                };
                // Chunked (M44/T44.5): the backspaces and the characters
                // together can exceed the host's queue, and a fill that
                // lost its tail used to blame the control for it.
                let before = host.dropped_inputs();
                let frame = feed(
                    host,
                    [Event::down(x, y), Event::up(x, y), Event::Key { key: Key::End, down: true }]
                        .into_iter()
                        .chain((0..existing).map(|_| Event::Key { key: Key::Backspace, down: true }))
                        .chain(text.chars().map(|ch| Event::Text { ch })),
                )
                .0;
                let dropped = lost(before, host.dropped_inputs());
                if dropped.is_some_and(|d| d > 0) {
                    return Err((
                        INVALID_PARAMS,
                        "the host's input queue dropped part of the fill, so the field holds \
                         something other than the text; see `dropped_inputs` in \
                         `app.diagnostics`"
                            .into(),
                        json!({ "dropped": dropped, "at": [x, y] }),
                    ));
                }

                // Did it take? Checked by reading the value back, not by
                // asking whether the node *looks* editable: a `Label` is
                // not, a `Button` is focusable and is not either, and a
                // field may clamp or filter what it accepts. Only the
                // outcome knows.
                //
                // Without this the method reported success for doing
                // nothing — found by running it against a `Label` in the
                // live window, where it answered `cleared: 1, chars: 3`
                // and changed not one character. A silent no-op is the
                // failure mode this whole surface is built to refuse.
                let got = {
                    let tree = host.tree();
                    let ix = Index::build(&tree);
                    resolve_one(&ix, selector_param(params)?)
                        .ok()
                        .and_then(|e| e.node.value().map(|v| v.to_string()))
                        .unwrap_or_default()
                };
                if got != text {
                    return Err((
                        INVALID_PARAMS,
                        "that control did not accept the text — it is not an editable field, \
                         or it rejected the value"
                            .into(),
                        json!({ "wanted": text, "got": got, "at": [x, y] }),
                    ));
                }
                Ok(json!({
                    "frame": self.reply_frame(),
                    "host_frame": frame,
                    "at": [x, y],
                    "cleared": existing,
                    "chars": n,
                }))
            }
            // Scroll a control into its scroller's viewport, through the
            // queue — the same reasoning as `input.fill`, and the
            // prerequisite for it: `input.click` resolves a selector to
            // the node's *centre*, and a centre outside the viewport is
            // a point the hit test correctly rejects, so a control that
            // has scrolled away cannot be reached at all.
            //
            // Named `input.` rather than `ui.` because it synthesizes
            // input: it is a recorded gesture, not a query.
            "input.scrollIntoView" => {
                let selector = selector_param(params)?.to_owned();
                let mut steps = 0u32;
                // Nearest scroller first, then outwards: an inner list
                // may have to move before the page can see the row at
                // all. Bounded, because each pass re-lays-out and a
                // clamped scroller that cannot move must not spin.
                for _ in 0..8 {
                    let step = {
                        let tree = host.tree();
                        let tree = &tree;
                        let ix = Index::build(tree);
                        let e = resolve_one(&ix, &selector)?;
                        let i = e.idx.ok_or((
                            NOT_FOUND,
                            "selector resolved to virtualized content".into(),
                            Value::Null,
                        ))?;
                        scroll_step(tree, i)
                    };
                    let Some((sx, sy, dy)) = step else { break };
                    host.enqueue(Event::Scroll { x: sx, y: sy, dy });
                    host.pump();
                    steps += 1;
                }
                let tree = host.tree();
                let tree = &tree;
                let ix = Index::build(tree);
                let e = resolve_one(&ix, &selector)?;
                let i = e.idx.unwrap_or(0);
                let st = &tree.states[i];
                let (cx, cy) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
                Ok(json!({
                    "steps": steps,
                    "reachable": reaches(tree, i, cx, cy),
                    "bounds": [st.pos.x, st.pos.y, st.pos.x + st.size.w, st.pos.y + st.size.h],
                }))
            }
            "input.type" => {
                let text = params.get("text").and_then(Value::as_str).ok_or((
                    INVALID_PARAMS,
                    "missing `text`".into(),
                    Value::Null,
                ))?;
                // One JSON character becomes one queued Event. When this
                // limit was written the queue had no cap, and 2,000,000
                // chars measured as 2,000,000 events in 51 ms; it has had
                // one since M13 — 64 slots — so the limit now bounds the
                // *work* one call can ask for, and the characters are fed
                // in chunks the queue can hold (M44/T44.5). A real
                // keyboard has a rate limit; so does this. It is also the
                // one agent method whose *availability* effect a user
                // with a keyboard could not reproduce, which is the
                // invariant the input surface is otherwise held to
                // (`docs/plan-review-fixes.md` F2.6).
                let n = text.chars().count();
                if n > MAX_TYPE_CHARS {
                    return Err((
                        INVALID_PARAMS,
                        format!("`text` is {n} characters; the limit is {MAX_TYPE_CHARS}"),
                        json!({ "chars": n, "limit": MAX_TYPE_CHARS }),
                    ));
                }
                // `chars` is what **arrived**: the characters sent less
                // any the host's queue dropped, which it counts. It used
                // to be the length of the request whatever happened.
                let before = host.dropped_inputs();
                let (f, h) = feed(host, text.chars().map(|ch| Event::Text { ch }));
                let dropped = lost(before, host.dropped_inputs());
                let delivered = n.saturating_sub(dropped.unwrap_or(0) as usize);
                Ok(json!({ "frame": self.reply_frame(), "host_frame": f, "handled": h, "chars": delivered, "sent": n, "dropped": dropped }))
            }
            // **A paste is one edit** — M45/T45.7. The string reaches the
            // focused node whole, through its `Node::paste`, and the
            // answer is what it kept: a `TextArea` keeps line breaks, a
            // `TextInput` drops them. `input.type` is the other thing — a
            // keystroke per character — and a recorded session keeps the
            // two apart, so a paste replays as a paste.
            "input.paste" => {
                let text = params.get("text").and_then(Value::as_str).ok_or((
                    INVALID_PARAMS,
                    "missing `text`".into(),
                    Value::Null,
                ))?;
                let n = text.chars().count();
                if n > MAX_TYPE_CHARS {
                    return Err((
                        INVALID_PARAMS,
                        format!("`text` is {n} characters; the limit is {MAX_TYPE_CHARS}"),
                        json!({ "chars": n, "limit": MAX_TYPE_CHARS }),
                    ));
                }
                let (f, kept) = host.paste(text).ok_or((
                    INVALID_REQUEST,
                    "this host cannot paste".into(),
                    json!({ "supported": false }),
                ))?;
                Ok(json!({ "frame": f, "kept": kept, "chars": n }))
            }
            _ => Err((METHOD_NOT_FOUND, format!("unknown method `{method}`"), Value::Null)),
        }
    }

    /// `{x, y}` directly, or the centre of whatever `selector` resolves
    /// to — so agents can click by author id and never compute geometry.
    fn point_param(
        &self,
        host: &dyn AgentHost,
        params: &Value,
    ) -> Result<(f32, f32), (i64, String, Value)> {
        if let (Some(x), Some(y)) =
            (params.get("x").and_then(Value::as_f64), params.get("y").and_then(Value::as_f64))
        {
            return Ok((coord(x, "x")?, coord(y, "y")?));
        }
        let tree = host.tree();
        let tree = &tree;
        let ix = Index::build(tree);
        let e = resolve_one(&ix, selector_param(params)?)?;
        // **A pointer wants a point, not a table index** (M27/T27.0).
        // This required `idx` solely to reach `states[i]` for a centre
        // the entry's own bounds already carry — so every click, hover
        // and held press on a repeater row was refused, and the refusal
        // told the caller to compute coordinates instead. That advice is
        // the vacuous shape AGENT.md names, and `scripts/agent_drive.sh`
        // took it.
        //
        // The table path stays first: `states[i]` is what the hit test
        // will itself consult, and bounds are a `f64` round-trip through
        // AccessKit. Identical in practice, but there is no reason to
        // introduce a conversion for the case that never needed one.
        if let Some(i) = e.idx {
            let st = &tree.states[i];
            return Ok((st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0));
        }
        let b = e.bounds().ok_or((
            NOT_FOUND,
            "selector resolved to a node with no bounds; pass x/y instead".into(),
            Value::Null,
        ))?;
        Ok((((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
    }

    fn handle(&self, tree: &TreeRef, method: &str, params: &Value) -> RpcResult {
        match method {
            "ui.getTree" => Ok(self.get_tree(tree)),
            "ui.getLayout" => self.get_layout(tree, params),
            "ui.resolve" => {
                let ix = Index::build(tree);
                let e = resolve_one(&ix, selector_param(params)?)?;
                Ok(ix.summary(e))
            }
            "ui.explain" => self.explain(tree, params),
            "ui.getAppliedStyles" => self.applied_styles(tree, params),
            "ui.probe" => self.probe(tree, params),
            "ui.probeRegion" => self.probe_region(tree, params),
            "ui.lint" => Ok(self.lint(tree)),
            "app.logs" => Ok(self.logs(params)),
            "ui.lastChange" => Ok(self.last_change(tree, params)),
            "ui.lastDamage" => Ok(self.last_damage(params)),
            "app.diagnostics" => Ok(self.diagnostics(tree)),
            // A tree alone cannot say whether anything is moving: that is
            // host state (`AgentHost::settled`), which `dispatch_input`
            // asks before it gets here. **"Cannot say" is `null`**, never
            // the `true` that makes a poll pass (M48/T48.2, R13-F1).
            "ui.waitSettled" => Ok(json!({
                "settled": Value::Null,
                "frame": self.frame,
                "reason": "this host cannot say whether anything is still moving",
            })),
            "app.perf" => Ok(self.perf(params)),
            _ => Err((METHOD_NOT_FOUND, format!("unknown method `{method}`"), Value::Null)),
        }
    }

    /// `ui.getTree`: the walk is `observe::emit_root` (M35/T35.1) and the
    /// JSON is `observe::tree_json` (M48/T48.5) — the same two functions
    /// `observe::dump` calls, so the agent's tree and the dump cannot
    /// drift. A private copy of the serialiser lived here until T48.5,
    /// and both copies dropped every state field a widget publishes.
    fn get_tree(&self, tree: &TreeRef) -> Value {
        let mut nodes = Vec::with_capacity(tree.table.len());
        observe::emit_root(tree.table, tree.root, tree.states, &|i| (i + 1) as u64, &mut nodes);
        observe::tree_json(tree.table, &nodes)
    }

    fn get_layout(&self, tree: &TreeRef, params: &Value) -> RpcResult {
        let ix = Index::build(tree);
        let e = resolve_one(&ix, selector_param(params)?)?;
        let mut out = ix.summary_map(e);
        if let Some(i) = e.idx {
            let st = &tree.states[i];
            out.insert(
                "constraints".into(),
                json!({
                    "min_w": st.constraints.min_w, "max_w": st.constraints.max_w,
                    "min_h": st.constraints.min_h, "max_h": st.constraints.max_h,
                }),
            );
            out.insert("size".into(), json!([st.size.w, st.size.h]));
            out.insert("pos".into(), json!([st.pos.x, st.pos.y]));
        }
        Ok(Value::Object(out))
    }

    /// Uses the core hit test (plan E1), not the AccessKit snapshot —
    /// the same code path input dispatch takes, so probe and click can
    /// never disagree about what is under a point.
    /// `ui.explain` — why a click on this node would or would not land.
    ///
    /// Deferred at P4 until "there is event routing to narrate"
    /// (`docs/plan-agent-parity.md`); M9 built the routing, so this is
    /// the narration. The question it answers is the one an agent
    /// actually has — *"I clicked #save and nothing happened"* — and the
    /// method by which it answers is the point: it runs the **real hit
    /// test** at the node's own centre and reports what came back. A
    /// narrative assembled from properties would be a second model of
    /// dispatch, free to drift from the first; asking the same
    /// `pass::hit_test` a click asks cannot.
    ///
    /// `at` overrides the probe point, for "why did my click at *these*
    /// coordinates go somewhere else".
    fn explain(&self, tree: &TreeRef, params: &Value) -> RpcResult {
        let ix = Index::build(tree);
        let e = resolve_one(&ix, selector_param(params)?)?;
        let Some(i) = e.idx else {
            return Err((
                INVALID_PARAMS,
                "that selector names a synthesized instance node, which has no table entry \
                    to explain; probe its coordinates instead"
                    .into(),
                Value::Null,
            ));
        };
        let st = &tree.states[i];
        let style = &tree.styles[i];
        let (w, h) = (st.size.w, st.size.h);

        let (px, py) = match params.get("at").and_then(Value::as_array) {
            Some(a) if a.len() == 2 => (
                coord(a[0].as_f64().unwrap_or(f64::NAN), "at[0]")?,
                coord(a[1].as_f64().unwrap_or(f64::NAN), "at[1]")?,
            ),
            _ => (st.pos.x + w / 2.0, st.pos.y + h / 2.0),
        };

        let hit = pass::hit_test(tree.root, tree.states, px, py);
        // Is the node that was hit this one, or inside it? The table is
        // pre-order, so "inside" is an ancestor walk.
        let mut reached = None;
        if let Some(hh) = hit {
            let mut k = hh.node as usize;
            loop {
                if k == i {
                    reached = Some(hh.node);
                    break;
                }
                if tree.table[k].parent == lumen_static::desc::NO_PARENT {
                    break;
                }
                k = tree.table[k].parent as usize;
            }
        }

        let mut reasons: Vec<Value> = Vec::new();
        let mut push = |code: &str, detail: String| {
            reasons.push(json!({ "code": code, "detail": detail }));
        };
        if !style.visible {
            push(
                "hidden",
                "`Style::visible` is false: the subtree is laid out at zero size, painted \
                 nothing and rejected by hit testing"
                    .into(),
            );
        }
        if w <= 0.0 || h <= 0.0 {
            push(
                "no-area",
                format!(
                    "laid out {w:.0}x{h:.0}. A node with no area cannot be hit, whatever is \
                        inside it — check the parent's constraints, or `ui.lint` for \
                        W1001/W1005"
                ),
            );
        }
        if !tree.table[i].is_focusable() {
            push(
                "not-focusable",
                "this node takes no keyboard focus, so `input.key` will never reach it; \
                    pointer input still can"
                    .into(),
            );
        }
        match (hit, reached) {
            (None, _) => push(
                "nothing-there",
                format!(
                    "hit testing at [{px:.0},{py:.0}] found no node at all — the point is \
                    outside the root, or every candidate has zero area"
                ),
            ),
            (Some(hh), None) => {
                let other = hh.node as usize;
                let os = &tree.states[other];
                push(
                    "occluded",
                    format!(
                        "the point belongs to node {} (`{}`{}), laid out \
                            [{:.0},{:.0},{:.0},{:.0}] — it is in front of, or clipped over, \
                            the one you asked about",
                        other,
                        tree.seeds[other].kind,
                        if tree.table[other].wid.is_empty() {
                            String::new()
                        } else {
                            format!(" #{}", tree.table[other].wid)
                        },
                        os.pos.x,
                        os.pos.y,
                        os.pos.x + os.size.w,
                        os.pos.y + os.size.h,
                    ),
                );
            }
            (Some(_), Some(_)) => {}
        }

        let reachable = reached.is_some() && reasons.is_empty();
        let summary = if reachable {
            format!("a click at [{px:.0},{py:.0}] reaches this node")
        } else if reached.is_some() {
            format!(
                "a click at [{px:.0},{py:.0}] reaches this node, but {} condition(s) may \
                    still make it useless",
                reasons.len()
            )
        } else {
            format!("a click at [{px:.0},{py:.0}] does NOT reach this node")
        };

        Ok(json!({
            "summary": summary,
            "reachable": reachable,
            "probed_at": [px, py],
            "target": ix.summary(e),
            "hit": hit.map(|hh| self.hit_summary(tree, &hh)),
            "reasons": reasons,
        }))
    }

    /// `ui.getAppliedStyles` — the cascade's answer for one node.
    ///
    /// The other half of "why does it look like this". `src` provenance
    /// says *where the node came from*; this says what the stylesheet
    /// resolved for it, which is the question a wrong colour or a
    /// surprising size actually raises. Reported from the same `Style`
    /// row layout reads, not from a re-run of the cascade.
    fn applied_styles(&self, tree: &TreeRef, params: &Value) -> RpcResult {
        let ix = Index::build(tree);
        let e = resolve_one(&ix, selector_param(params)?)?;
        let Some(i) = e.idx else {
            return Err((
                INVALID_PARAMS,
                "instances share their template's style row; ask for the template".into(),
                Value::Null,
            ));
        };
        let s = &tree.styles[i];
        let st = &tree.states[i];
        let color = |c: lumen_static::Color| json!([c.0, c.1, c.2, c.3]);
        Ok(json!({
            "index": i,
            "width": format!("{:?}", s.width),
            "height": format!("{:?}", s.height),
            "padding": [s.padding.left, s.padding.top, s.padding.right, s.padding.bottom],
            "spacing": s.spacing,
            "color": color(s.color),
            "background": color(s.background),
            "radius": s.radius,
            "font_size": s.font_size,
            "font_weight": s.font_weight,
            "shadow": s.shadow,
            // M27/T27.1. Reported because a drive script has to be able
            // to ask "did the sheet's border reach this node" without
            // reading pixels — and because a `Style` field the agent
            // cannot see is a field no lint and no drive can check.
            "border_width": s.border_width,
            "border_color": color(s.border_color),
            "align": format!("{:?}", s.align),
            "visible": s.visible,
            // The interaction state that changes what is painted (C6),
            // so a screenshot taken mid-gesture can be explained.
            "state": {
                "pressed": st.pressed,
                "hovered": st.hovered,
                "focused": st.focused,
                "painted_background": color(st.bg),
            },
        }))
    }

    fn probe(&self, tree: &TreeRef, params: &Value) -> RpcResult {
        let (x, y) = (coord(f64_param(params, "x")?, "x")?, coord(f64_param(params, "y")?, "y")?);
        match pass::hit_test(tree.root, tree.states, x, y) {
            Some(h) => Ok(self.hit_summary(tree, &h)),
            None => Ok(Value::Null),
        }
    }

    /// A hit rendered with the same node ids `observe` uses, so agents can
    /// carry ids between `ui.getTree` and `ui.probe`.
    fn hit_summary(&self, tree: &TreeRef, h: &Hit) -> Value {
        let mut o = self.node_ident(tree, h.node);
        let st = &tree.states[h.node as usize];
        o.insert(
            "bounds".into(),
            json!([st.pos.x, st.pos.y, st.pos.x + st.size.w, st.pos.y + st.size.h]),
        );
        match h.inner {
            // Through `observe::instance_id`, not a fourth hand-written
            // copy of the bit layout — the comment here used to say it
            // mirrored a Repeat method deleted in M0,
            // and nothing checked the copies agreed (F4.4).
            Some(inn) => {
                let id = lumen_static::observe::instance_id(
                    h.node as u64 + 1,
                    inn.row,
                    inn.node as usize,
                );
                o.insert("id".into(), id.into());
                o.insert("row".into(), inn.row.into());
                o.insert("inner".into(), inn.node.into());
            }
            None => {
                o.insert("id".into(), (h.node as u64 + 1).into());
            }
        }
        Value::Object(o)
    }

    fn probe_region(&self, tree: &TreeRef, params: &Value) -> RpcResult {
        let (x, y) = (f64_param(params, "x")?, f64_param(params, "y")?);
        let (w, h) = (f64_param(params, "w")?, f64_param(params, "h")?);
        let ix = Index::build(tree);
        let hits: Vec<Value> = ix
            .entries
            .iter()
            .filter(|e| e.intersects(x, y, w, h))
            .take(100)
            .map(|e| ix.summary(e))
            .collect();
        Ok(Value::Array(hits))
    }

    /// P3: conditions currently true of the frame. Reads the set the
    /// audit computed at the last frame boundary, so the query is free.
    /// How many findings the last frame carried — for benchmarks that
    /// need to state the shape of the tree they measured.
    pub fn finding_count(&self) -> usize {
        self.auditor.current().len()
    }

    fn lint(&self, tree: &TreeRef) -> Value {
        let mut findings: Vec<Value> = self
            .auditor
            .current()
            .iter()
            .map(|f| {
                let mut o = self.node_ident(tree, f.idx);
                o.insert("code".into(), f.code.into());
                o.insert("message".into(), f.message().into());
                o.insert("detail".into(), f.detail());
                Value::Object(o)
            })
            .collect();
        // W1010 (M25/T25.2): per *rule*, not per node — the rule is what
        // is wrong, and an unmatched one has no node to name.
        for f in tree.sheet_findings {
            let Some(rule) = tree.sheet.get(f.rule) else { continue };
            let selector = format!("{:?}", rule.selector);
            let (message, fields) = match f.issue {
                lumen_static::style::SheetIssue::Unmatched => (
                    format!(
                        "stylesheet rule {} ({selector}) matches no node — check its spelling \
                         against the tree's kinds and ids (built-in kinds are lowercase)",
                        f.rule
                    ),
                    Vec::new(),
                ),
                lumen_static::style::SheetIssue::Unread(set) => {
                    let names: Vec<&str> = set.names().collect();
                    (
                        format!(
                            "stylesheet rule {} ({selector}) sets {} but no node it matches reads it",
                            f.rule,
                            names.join(", ")
                        ),
                        names,
                    )
                }
            };
            findings.push(json!({
                "code": audit::codes::SHEET_NO_EFFECT,
                "rule": f.rule,
                "selector": selector,
                "message": message,
                "detail": { "fields": fields },
            }));
        }
        // W1011 (M27/T27.3): two nodes with the same author id. Static —
        // it reads the const table and nothing else — so it is computed
        // here rather than by the per-frame auditor.
        // **Scoped, since M27/T27.0's third landing.** A duplicate is
        // only *unaddressable* when a descendant step cannot separate
        // the two — that is, when they share the nearest enclosing node
        // that has an id of its own. Two `#chooser-trigger`s under
        // `#a` and `#b` are both reachable as `#a > #chooser-trigger`
        // and `#b > #chooser-trigger`, and reporting them would be the
        // lint crying wolf at the pattern the step exists to support.
        //
        // Two under the *same* owner still cannot be told apart, and
        // that is what this now reports.
        let scope_of = |mut k: usize| -> usize {
            loop {
                let p = tree.table[k].parent;
                if p == lumen_static::desc::NO_PARENT {
                    return usize::MAX;
                }
                if !tree.table[p as usize].wid.is_empty() {
                    return p as usize;
                }
                k = p as usize;
            }
        };
        let mut seen: Vec<(&str, usize, usize)> = Vec::new();
        for (i, d) in tree.table.iter().enumerate() {
            if d.wid.is_empty() {
                continue;
            }
            let scope = scope_of(i);
            match seen.iter().find(|(w, s, _)| *w == d.wid && *s == scope) {
                Some(&(_, _, first)) => {
                    let mut o = self.node_ident(tree, i as u32);
                    o.insert("code".into(), audit::codes::DUPLICATE_ID.into());
                    o.insert(
                        "message".into(),
                        format!(
                            "author id `{}` is on node {i} and node {first}, both inside \
                             the same owner — neither is addressable: `#{}` resolves to \
                             neither, and not even `#owner > #{}` can tell them apart",
                            d.wid, d.wid, d.wid
                        )
                        .into(),
                    );
                    o.insert("detail".into(), json!({ "wid": d.wid, "first": first }));
                    findings.push(Value::Object(o));
                }
                None => seen.push((d.wid, scope, i)),
            }
        }
        json!({ "frame": self.frame, "findings": findings })
    }

    /// P3's push half: transitions, paged like Lumen's `app.logs`.
    fn logs(&self, params: &Value) -> Value {
        let since = self.since_param(params);
        let (entries, truncated) = self.auditor.log_since(since);
        let entries: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "frame": e.frame,
                    "event": e.event,
                    "code": e.code,
                    "index": e.idx,
                    "message": e.message,
                })
            })
            .collect();
        json!({ "frame": self.frame, "since": since, "entries": entries, "truncated": truncated })
    }

    /// The identity block every node-bearing response carries: index,
    /// kind, author id, and the source line to edit.
    fn node_ident(&self, tree: &TreeRef, idx: u32) -> Map<String, Value> {
        let mut o = Map::new();
        let i = idx as usize;
        o.insert("index".into(), i.into());
        if let Some(d) = tree.table.get(i) {
            o.insert("kind".into(), tree.seeds[i].kind.into());
            if !d.wid.is_empty() {
                o.insert("wid".into(), d.wid.into());
            }
            if !d.src.file.is_empty() {
                o.insert("src".into(), format!("{}:{}", d.src.file, d.src.line).into());
            }
        }
        o
    }

    /// `since` defaults to the previous frame, so a bare call means
    /// "what changed in the last frame".
    ///
    /// **The contract with `input.*`** — M48/T48.1 (R13-F3): a reply's
    /// `frame` is the first journal frame that includes its input
    /// ([`Agent::reply_frame`]), so `since = frame - 1` finds what the
    /// input changed once that frame has been drawn.
    fn since_param(&self, params: &Value) -> u64 {
        params.get("since").and_then(Value::as_u64).unwrap_or_else(|| self.frame.saturating_sub(1))
    }

    /// P2: what changed since a frame, from the state diff — the
    /// agent-facing purpose of Lumen's dependency-graph queries, without
    /// a dependency graph.
    fn last_change(&self, tree: &TreeRef, params: &Value) -> Value {
        let since = self.since_param(params);
        let report = self.journal.since(since);
        let changed: Vec<Value> = report
            .changed
            .iter()
            .map(|c| {
                let mut o = self.node_ident(tree, c.idx);
                o.insert("change".into(), change_kind(c).into());
                o.insert(
                    "old".into(),
                    json!({ "size": [c.old.size.w, c.old.size.h], "pos": [c.old.pos.x, c.old.pos.y] }),
                );
                o.insert(
                    "new".into(),
                    json!({ "size": [c.new.size.w, c.new.size.h], "pos": [c.new.pos.x, c.new.pos.y] }),
                );
                Value::Object(o)
            })
            .collect();
        json!({
            "frame": self.frame,
            "since": since,
            "changed": changed,
            "truncated": report.truncated,
            "reset": report.reset,
            "oldest_frame": report.oldest_frame,
        })
    }

    /// The union of everything that changed — Lumen's `ui.lastDamage`,
    /// derived from the same diff rather than from paint bookkeeping.
    ///
    /// **Logical pixels**, because the journal is `NodeState` geometry.
    /// It is which *nodes* changed, not what the surface redrew: that is
    /// retention's, in physical pixels (M40/T40.1), and it reaches the
    /// agent only as `app.perf`'s `damaged_px`. The two agree at a scale
    /// of 1.0 and nowhere else (`docs/damage-amplification-decision.md`).
    fn last_damage(&self, params: &Value) -> Value {
        let since = self.since_param(params);
        let report = self.journal.since(since);
        let rect = journal::damage(&report.changed);
        let regions = journal::damage_regions(&report.changed);
        json!({
            "frame": self.frame,
            "since": since,
            // **`rect` is the bounding box of `regions`, and is the
            // weaker answer.** Kept because it is what every existing
            // caller reads, but a bound computed against it is a bound
            // against the wrong quantity: two small changes at opposite
            // corners bound to the whole window. Use `regions`.
            "rect": rect.map(|r| json!([r[0], r[1], r[2], r[3]])).unwrap_or(Value::Null),
            "regions": regions.as_slice().iter()
                .map(|r| json!([r[0], r[1], r[2], r[3]]))
                .collect::<Vec<_>>(),
            "nodes": report.changed.len(),
            "truncated": report.truncated,
            "reset": report.reset,
        })
    }

    /// `app.diagnostics`: the frame, node counts, repeaters and byte
    /// sizes — and `process`, which a tree alone answers with
    /// `{"supported": false}` and a host replaces with
    /// [`AgentHost::process_info`] (pid, exe, strategy, features; M42).
    fn diagnostics(&self, tree: &TreeRef) -> Value {
        let ix = Index::build(tree);
        let rigid = tree.table.iter().filter(|d| d.is_rigid()).count();
        let containers = tree.table.iter().filter(|d| d.is_container()).count();
        let repeaters: Vec<Value> = (0..tree.table.len())
            .filter_map(|i| {
                let mut s = lumen_static::desc::NodeStats::default();
                tree.root.node(i).stats(&mut s);
                s.items.map(|items| {
                    json!({
                        "index": i,
                        "rows": items,
                        "window": s.materialized.unwrap_or(0),
                    })
                })
            })
            .collect();
        json!({
            "frame": self.frame,
            "nodes": tree.table.len(),
            "total_nodes": ix.entries.len(),
            "rigid": rigid,
            "containers": containers,
            "repeaters": repeaters,
            "table_bytes": std::mem::size_of_val(tree.table),
            "state_bytes": std::mem::size_of_val(tree.states),
            // A tree alone cannot say which process it is in; the
            // host-aware branch of `dispatch_input` replaces this with
            // `AgentHost::process_info` (M42/T42.1).
            "process": { "supported": false },
        })
    }
}

/// One past the last table node under `i`: descendants follow their
/// ancestor contiguously in the table, so this walks forward until a node
/// whose parent chain does not reach `i` (M42/T42.6, `ui.resolve`'s
/// `subtree`).
fn subtree_end(table: &[lumen_static::desc::NodeDesc], i: usize) -> usize {
    let under = |mut j: usize| loop {
        let p = table[j].parent;
        if p == lumen_static::desc::NO_PARENT || (p as usize) < i {
            return false;
        }
        if p as usize == i {
            return true;
        }
        j = p as usize;
    };
    let mut end = i + 1;
    while end < table.len() && under(end) {
        end += 1;
    }
    end
}

fn change_kind(c: &Coalesced) -> &'static str {
    match (c.geometry, c.content) {
        (true, true) => "both",
        (true, false) => "geometry",
        _ => "content",
    }
}

/// The mutable host surface the `input.*` methods need.
///
/// It hands out the tree view as well as taking events, because the tree
/// lives *inside* the host: a caller cannot hold a `TreeRef` borrowed
/// from the host and pass `&mut host` at the same time. Handing the view
/// out per call keeps each read borrow short and ends it before anything
/// mutates.
///
/// Synthesized input enters the same queue as OS input, and `pump` is the
/// host's ordinary frame step — so nothing an agent does can take a path
/// a real user could not.
/// What `input.*` needs from whatever is driving the frames.
///
/// Named `AgentHost`, not `Host`: `lumen_static_shell::Host<A>` is a
/// different thing at the same layer, and two public types sharing a
/// name is a documentation hazard even before anything implements both
/// (`docs/plan-review-fixes.md` F6.5).
///
/// **Nothing in the workspace implements this outside a test mock**, and
/// that is the finding rather than the trait's shape. The shell's `Host`
/// has all three of these — `post`, `pump`, and the fields to build a
/// `TreeRef` — but cannot implement it, because the shell does not
/// depend on this crate and adding the dependency would unify `observe`
/// into every binary linking the shell (F2.5). So `examples/counter`
/// reaches into three public fields to rebuild by hand the value
/// `AgentHost::tree` exists to return, and never exercises `input.*` at
/// all. Closing that needs the trait to move to the core, or a bridge
/// crate; both are larger than a rename.
pub trait AgentHost {
    fn tree(&self) -> TreeRef<'_>;

    /// Physical pixels per logical pixel (M10b).
    ///
    /// Everything this protocol reports as geometry is **logical**;
    /// `ui.screenshot` returns **physical** pixels. A client that
    /// compares one to the other — `scripts/ui_check.py` does, for
    /// centring, edges and contrast — must scale, and cannot unless the
    /// host says by how much. Reported through `app.diagnostics` and
    /// alongside every screenshot.
    fn scale(&self) -> f32 {
        1.0
    }
    /// The last rendered frame as RGBA8, if this host renders. `None`
    /// for a host that only lays out — which is what `ui.screenshot`
    /// reports as an error rather than a blank image.
    fn frame(&self) -> Option<(u32, u32, &[u8])> {
        None
    }

    /// The pixels **actually on screen**, read back from the window's
    /// surface, if this host can.
    ///
    /// Not the same thing as [`AgentHost::frame`], and the difference is
    /// the point. `frame` returns the renderer's canvas — what the app
    /// *believes* it drew. This returns what the display server holds.
    /// Everything between the two (the blit, the damage rects, the
    /// present, the buffer's age) is invisible to every other method
    /// here, and on 2026-09-04 a bug in exactly that gap put another
    /// window's pixels on screen while `ui.screenshot`, `ui.lint` and
    /// every live check reported a perfect frame.
    ///
    /// `None` where a platform cannot read a window back (softbuffer
    /// implements it on X11 and Web only), so a caller must treat an
    /// absent answer as "not checkable here", never as "matches".
    fn presented(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
    fn enqueue(&mut self, ev: Event);
    /// Drain the queue, re-lay out, record the frame.
    ///
    /// Returns the frame's number **and whether any node took an event**
    /// — `Host::pump`'s own answer, which this discarded until
    /// M27/T27.11.
    ///
    /// The second half is the only direct answer to "did that input
    /// land". Everything else a drive can ask is a *consequence*: a
    /// changed value, a damaged rect, a different readout — and an
    /// action whose correct outcome is that nothing happens has no
    /// consequence to check, so "it landed" and "it did nothing" were
    /// indistinguishable. `Tree`'s "pressing a leaf does nothing" stayed
    /// green under an injection that made a leaf toggle, because a leaf
    /// has no children and folding its bit changes nothing anyone can
    /// see. It reports, and now that is visible.
    ///
    /// **Required, not provided.** A default of `(self.pump(), false)`
    /// would make every test double answer "nothing was handled" and be
    /// believed — the provided-method trap `Deferring` fell into, one
    /// trait over.
    fn pump(&mut self) -> (u64, bool);

    /// Paste `text` into the focused node as one edit, then draw the
    /// frame — M45/T45.7, the `input.paste` method. Returns the frame's
    /// number and the characters the node kept (`Host::paste_with`).
    ///
    /// `None` by default, for a host that cannot paste, which the method
    /// reports as an error rather than as "kept nothing": a drive that
    /// asserted an empty paste against a host that never pasted would be
    /// proving nothing.
    fn paste(&mut self, text: &str) -> Option<(u64, usize)> {
        let _ = text;
        None
    }

    /// Swap the palette looks are resolved against — M25/T25.3, the
    /// `app.setPalette` method.
    ///
    /// Returns whether the host applied it. `false` by default, for a host
    /// with no palette to set, which the method reports rather than
    /// pretending: a provided method that silently succeeds is how an
    /// agent comes to believe a window changed when it did not.
    fn set_palette(&mut self, _palette: lumen_static::style::Palette) -> bool {
        false
    }

    /// **Whether anything is still moving** — M48/T48.2, the
    /// `ui.waitSettled` method (`docs/plan-agent-truth.md` §2.2).
    ///
    /// An object with `settled` and each term that decided it —
    /// `animating`, `more_deferred`, `pending_tasks`,
    /// `next_deadline_ms`, `subscriptions` — so a caller told "no" learns
    /// why. The agent adds `frame`.
    ///
    /// **`None` by default, which the method reports as `settled: null`:
    /// "this host cannot say".** Not `false`, which would make every poll
    /// spin to its timeout, and not `true`, which is the defect this
    /// replaced: `ui.waitSettled` answered `true` in every state under a
    /// comment saying no animation scheduler existed, long after
    /// `App::tick` and the task inbox made one (R13-F1). Provided rather
    /// than required for the reason [`AgentHost::process_info`] gives:
    /// every implementor is a leaf holding a host, and the default says
    /// it does not know.
    fn settled(&mut self) -> Option<Value> {
        None
    }

    /// The shadow oracle's report — M34/T34.1, the `app.shadow` method.
    ///
    /// `None` by default, for a host built without the oracle, which the
    /// method reports as `supported: false` rather than as "no
    /// divergence": a drive that asserted "quiet" against a host that
    /// never compared anything would be proving nothing.
    fn shadow(&self) -> Option<Value> {
        None
    }

    /// The panic being shown, if any — M36/T36.3, the `app.failure`
    /// method. `{"supported": false}` by default, for a host that cannot
    /// recover, so a drive does not mistake "cannot say" for "no panic".
    fn failure(&self) -> Value {
        json!({ "supported": false })
    }

    /// Retry after a panic — the `app.retry` method. `false` by default.
    fn retry(&mut self) -> bool {
        false
    }

    /// Which process this is — M42/T42.1, the `process` block of
    /// `app.diagnostics`: `{"supported": true, "strategy", "features",
    /// "pid", "exe"}`, built by [`process_block`].
    ///
    /// **Provided**, with `{"supported": false}` as its default, and that
    /// is safe here for a reason the provided-method rule names: every
    /// implementation of this trait is a *leaf* that holds a host, not a
    /// wrapper that could forward, and the drive harness refuses
    /// `supported: false` (`scripts/live/launch.py`, rule 2 of
    /// `docs/plan-drive-identity.md` §0). A window that forgets to
    /// implement it fails loudly on its first drive.
    ///
    /// It exists because a drive could not tell which process it drove:
    /// the strategy was a label the script set for itself, and a leftover
    /// window holding the port answered for a build that never bound
    /// (R18-F1).
    fn process_info(&self) -> Value {
        json!({ "supported": false })
    }

    /// How many input events the host's queue has ever dropped —
    /// M44/T44.5, reported in `app.diagnostics` and used by `input.type`
    /// and `input.fill` to say what actually arrived. `None` by default,
    /// for a host that cannot say, which the replies report as `null`
    /// rather than as zero.
    fn dropped_inputs(&self) -> Option<u32> {
        None
    }
}

/// The `process` block an [`AgentHost::process_info`] returns: the host's
/// invalidation strategy (its `type_name`), the features the build was
/// compiled with, and the pid and executable **as the process itself
/// reports them** — which is the point: a label the drive script sets for
/// itself proves nothing (M42/T42.1).
pub fn process_block(strategy: &str, features: &[&str]) -> Value {
    json!({
        "supported": true,
        "strategy": strategy,
        "features": features,
        "pid": std::process::id(),
        "exe": std::env::current_exe().ok().map(|p| p.display().to_string()),
    })
}

/// The keys `input.key` names. PageUp and PageDown since M44/T44.10: a
/// list's active row and a scroll view both answer them, and a drive that
/// could not press them could not check either.
/// `input.key`'s `mods`: a list of modifier names (M45/T45.6).
fn parse_mods(v: &Value) -> Result<lumen_static::event::Mods, (i64, String, Value)> {
    use lumen_static::event::Mods;
    let bad = || (INVALID_PARAMS, "`mods` is a list of \"shift\", \"ctrl\", \"alt\", \"meta\"".into(), v.clone());
    let mut m = Mods::NONE;
    for name in v.as_array().ok_or_else(bad)? {
        m = m.with(match name.as_str() {
            Some("shift") => Mods::SHIFT,
            Some("ctrl") => Mods::CTRL,
            Some("alt") => Mods::ALT,
            Some("meta") => Mods::META,
            _ => return Err(bad()),
        });
    }
    Ok(m)
}

const KEY_NAMES: [&str; 14] = [
    "Enter",
    "Space",
    "Escape",
    "Tab",
    "Backspace",
    "Delete",
    "Left",
    "Right",
    "Up",
    "Down",
    "Home",
    "End",
    "PageUp",
    "PageDown",
];

fn parse_key(name: &str) -> Option<Key> {
    Some(match name {
        "Enter" => Key::Enter,
        "Space" => Key::Space,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Home" => Key::Home,
        "End" => Key::End,
        _ => return None,
    })
}

// ---- snapshot index ---------------------------------------------------------

#[derive(Clone)]
struct Entry {
    id: u64,
    node: accesskit::Node,
    /// Main-table index, when this is not a synthesized instance node.
    idx: Option<usize>,
}

impl Entry {
    fn bounds(&self) -> Option<accesskit::Rect> {
        self.node.bounds()
    }
    fn intersects(&self, x: f64, y: f64, w: f64, h: f64) -> bool {
        self.bounds().is_some_and(|b| b.x0 < x + w && b.x1 > x && b.y0 < y + h && b.y1 > y)
    }
}

struct Index<'a> {
    tree: &'a TreeRef<'a>,
    entries: Vec<Entry>,
}

impl<'a> Index<'a> {
    fn build(tree: &'a TreeRef<'a>) -> Self {
        let mut nodes = Vec::with_capacity(tree.table.len());
        observe::emit_root(tree.table, tree.root, tree.states, &|i| (i + 1) as u64, &mut nodes);
        let entries: Vec<Entry> = nodes
            .into_iter()
            .map(|(nid, node)| {
                let idx = if nid.0 & (1 << 63) == 0 { Some((nid.0 - 1) as usize) } else { None };
                Entry { id: nid.0, node, idx }
            })
            .collect();

        Index { tree, entries }
    }

    fn summary_map(&self, e: &Entry) -> Map<String, Value> {
        let mut o = Map::new();
        o.insert("id".into(), e.id.into());
        o.insert("role".into(), format!("{:?}", e.node.role()).into());
        if let Some(v) = e.node.value() {
            o.insert("value".into(), v.into());
        }
        // **And the label.** `ui.getTree` has carried one since M15 and
        // this summary never did, so `ui.resolve` on a `Button` — which
        // publishes `set_label`, not `set_value` — answered with no text
        // at all. Anything reading a control's text through `resolve`
        // got the empty string and could not tell that from a control
        // with no text (M27/T27.9).
        if let Some(l) = e.node.label() {
            o.insert("label".into(), l.into());
        }
        if let Some(b) = e.bounds() {
            o.insert("bounds".into(), json!([b.x0, b.y0, b.x1, b.y1]));
        }
        // **And the state** — M48/T48.6 (R13-F4): `toggled`, `numeric`,
        // `expanded` and the rest, from the same function `ui.getTree`
        // uses (`observe::state_json`). This summary wrote a fixed set of
        // fields and dropped them, so `ui.resolve` on a checked box and
        // `ui.getLayout` on a moved slider could not tell.
        observe::state_json(&e.node, &mut o);
        if let Some(i) = e.idx {
            let d = &self.tree.table[i];
            o.insert("index".into(), i.into());
            // `[first, end)`: this node and every table node under it, which
            // the table stores contiguously. So a drive can ask whether a
            // change landed *on this widget* — `ui.lastChange` reports
            // indices — rather than anywhere in the frame (M42/T42.6).
            o.insert("subtree".into(), json!([i, subtree_end(self.tree.table, i)]));
            o.insert("kind".into(), self.tree.seeds[i].kind.into());
            if !d.wid.is_empty() {
                o.insert("wid".into(), d.wid.into());
            }
            if !d.src.file.is_empty() {
                o.insert("src".into(), format!("{}:{}", d.src.file, d.src.line).into());
            }
        }
        o
    }

    fn summary(&self, e: &Entry) -> Value {
        Value::Object(self.summary_map(e))
    }
}

// ---- selectors --------------------------------------------------------------

enum Term<'s> {
    Wid(&'s str),
    Kind(&'s str),
    /// `~text` — the text this node *shows* contains `text`.
    ///
    /// **The one term that reaches a repeater row** (M27/T27.0), because
    /// it matches the AccessKit node rather than the const table, and a
    /// row has no table entry of its own. It read `value()` alone until
    /// 2026-09-18, and `Button` publishes `set_label` — so `~Switch`
    /// missed a menu row by one field while matching the `Label` beside
    /// it. Both are "the text the user sees"; one term for both is the
    /// honest reading, and is why this is no longer called
    /// `ValueContains`.
    TextContains(&'s str),
    /// `~=text` — the text is the *whole* label or value, trimmed.
    ///
    /// **Because one name can be a prefix of another** (M27/T27.12).
    /// `~RichText` matched both `RichText` and `RichTextEditor` the
    /// moment the second existed, and "click the row named RichText"
    /// stopped being expressible — for the drive and for any agent
    /// driving the same window. Contains stays the default because it
    /// is what a person reaching for a selector means; this is the
    /// escape hatch for when it is not.
    TextIs(&'s str),
    /// `%k` — row *k* of the repeater this is inside (M24/T24.4).
    ///
    /// **The one thing a part id cannot reach.** An arena row is a
    /// template instantiated per index and carries no `#[widget_id]` of
    /// its own, so until this the only way to name one was the text it
    /// happened to show — safe exactly while that text is unique in the
    /// window — or arithmetic over a resolved box, which is the
    /// "setup silently did not happen" shape this repository has been
    /// caught by twice (`scripts/agent_drive.sh` computed
    /// `box[1] + ROW_H * 8 + ROW_H / 2`).
    ///
    /// Written with the owner: `#menu > %3` is the fourth row of
    /// `#menu`, and `%3` alone is every repeater's fourth row at once —
    /// which `ui.resolve` reports as ambiguous rather than guessing.
    Row(u64),
    Src { file: &'s str, line: Option<u32> },
    TableIndex(usize),
}

fn parse_selector(sel: &str) -> Result<Vec<Term<'_>>, String> {
    let mut terms = Vec::new();
    for raw in sel.split_whitespace() {
        let term = if let Some(id) = raw.strip_prefix('#') {
            Term::Wid(id)
        } else if let Some(k) = raw.strip_prefix('%') {
            Term::Row(
                k.parse().map_err(|_| format!("`{raw}`: a row is a number, as in `%3`"))?,
            )
        } else if let Some(text) = raw.strip_prefix("~=") {
            Term::TextIs(text)
        } else if let Some(text) = raw.strip_prefix('~') {
            Term::TextContains(text)
        } else if let Some(loc) = raw.strip_prefix('@') {
            match loc.rsplit_once(':') {
                Some((file, line)) if line.chars().all(|c| c.is_ascii_digit()) => Term::Src {
                    file,
                    line: Some(line.parse().map_err(|_| format!("bad line in `{raw}`"))?),
                },
                _ => Term::Src { file: loc, line: None },
            }
        } else if raw.chars().all(|c| c.is_ascii_digit()) {
            Term::TableIndex(raw.parse().map_err(|_| format!("bad index `{raw}`"))?)
        } else {
            Term::Kind(raw)
        };
        terms.push(term);
    }
    if terms.is_empty() {
        return Err("empty selector".into());
    }
    Ok(terms)
}

fn matches(tree: &TreeRef, e: &Entry, term: &Term) -> bool {
    match term {
        Term::TextContains(t) => {
            e.node.value().is_some_and(|v| v.contains(t))
                || e.node.label().is_some_and(|v| v.contains(t))
        }
        // Trimmed on the node's side only: a caller writing `~=Switch`
        // means the row that says "Switch", and the showcase's menu
        // indents its rows with spaces.
        Term::TextIs(t) => {
            e.node.value().is_some_and(|v| v.trim() == *t)
                || e.node.label().is_some_and(|v| v.trim() == *t)
        }
        Term::TableIndex(i) => e.idx == Some(*i),
        Term::Row(k) => instance_of(e.id).is_some_and(|(_, row)| row == *k),
        Term::Wid(_) | Term::Kind(_) | Term::Src { .. } => {
            let Some(i) = e.idx else { return false };
            let d = &tree.table[i];
            match term {
                Term::Wid(w) => d.wid == *w,
                Term::Kind(k) => tree.seeds[i].kind == *k,
                Term::Src { file, line } => {
                    d.src.file.ends_with(file) && line.is_none_or(|l| d.src.line == l)
                }
                _ => unreachable!(),
            }
        }
    }
}

/// What the error teaches when a term matched nothing.
fn nearest(tree: &TreeRef, term: &Term) -> Value {
    match term {
        Term::Wid(_) => {
            let ids: Vec<&str> =
                tree.table.iter().map(|d| d.wid).filter(|w| !w.is_empty()).collect();
            json!({ "available_ids": ids })
        }
        Term::Kind(_) => {
            let mut names: Vec<&str> = tree.seeds.iter().map(|s| s.kind).collect();
            names.sort_unstable();
            names.dedup();
            json!({ "available_kinds": names })
        }
        Term::Src { .. } => {
            let mut files: Vec<&str> = tree.table.iter().map(|d| d.src.file).collect();
            files.sort_unstable();
            files.dedup();
            json!({ "available_files": files })
        }
        Term::TableIndex(_) => json!({ "table_len": tree.table.len() }),
        Term::TextContains(_) | Term::TextIs(_) | Term::Row(_) => Value::Null,
    }
}

/// One scroll gesture that brings `i` closer to being visible, as
/// `(x, y, dy)` for a `Scroll` event — or `None` when every enclosing
/// scroller already shows it.
///
/// Only the *nearest* scroller that still needs to move is returned, and
/// the caller re-lays-out between steps: a scroll changes every position
/// below it, so computing a whole plan up front would be computing it
/// against geometry the first step invalidates.
fn scroll_step(tree: &TreeRef, i: usize) -> Option<(f32, f32, f32)> {
    let target = &tree.states[i];
    let (top, bottom) = (target.pos.y, target.pos.y + target.size.h);
    for s in scrolling_ancestors(tree, i) {
        let v = &tree.states[s];
        let (vt, vb) = (v.pos.y, v.pos.y + v.size.h);
        // `dy` is the wheel's sign: the offset moves *against* it, so
        // revealing content below needs a negative `dy`.
        let dy = if top < vt {
            vt - top
        } else if bottom > vb {
            vb - bottom
        } else {
            continue;
        };
        if dy.abs() < 0.5 {
            continue;
        }
        return Some((v.pos.x + v.size.w / 2.0, v.pos.y + v.size.h / 2.0, dy));
    }
    None
}

/// Whether a click at `(x, y)` would land on node `i` or inside it.
///
/// Asks the same `pass::hit_test` a real click asks, then walks the
/// parents of whatever came back. Shared by `ui.explain` and
/// `input.fill`, so the method that *diagnoses* an unreachable control
/// and the method that *refuses to type into one* cannot disagree.
fn reaches(tree: &TreeRef, i: usize, x: f32, y: f32) -> bool {
    let Some(h) = pass::hit_test(tree.root, tree.states, x, y) else { return false };
    let mut k = h.node as usize;
    loop {
        if k == i {
            return true;
        }
        if tree.table[k].parent == lumen_static::desc::NO_PARENT {
            return false;
        }
        k = tree.table[k].parent as usize;
    }
}

/// Scroll regions enclosing `i`, nearest first.
fn scrolling_ancestors(tree: &TreeRef, i: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut p = tree.table[i].parent;
    while p != lumen_static::desc::NO_PARENT {
        let k = p as usize;
        if tree.seeds[k].kind == "scrollview" {
            out.push(k);
        }
        p = tree.table[k].parent;
    }
    out
}

/// Is `j` inside `i`'s subtree? `false` when `j == i`.
///
/// Walks `parent` rather than comparing index ranges: a merged table
/// (`Either`, `Select`, every wrapper in `lumen-static-widgets`) splices
/// children in, so a subtree is contiguous *in practice* and the walk is
/// what makes that not need to be true.
/// The container node and row an [`lumen_static::observe::instance_id`]
/// names, or `None` for an ordinary table node.
///
/// The layout is that function's, read back: bit 63 marks an instance,
/// bits 44..=62 the container's main-table index, bits 12..=43 the row.
/// Kept next to the only two things that need it so the pair cannot
/// drift apart the way the four copies of the *encoder* did
/// (`docs/plan-review-fixes.md` F4.4).
fn instance_of(id: u64) -> Option<(usize, u64)> {
    // **The container is stored as its *node id*, not its index.**
    // `Repeat::observe` passes the id it was given and `Index::build`
    // hands out `i + 1`, so the table index is one less — and a zero
    // would mean no container at all, which cannot happen but is not
    // worth a panic to prove.
    (id >> 63 == 1).then(|| {
        let container = ((id >> 44) & 0x7_FFFF) as usize;
        (container.saturating_sub(1), (id >> 12) & 0xFFFF_FFFF)
    })
}

/// Is `j` inside `i`'s subtree — where `j` may be an **arena row**,
/// which has no table index at all.
///
/// A row's owner is the repeater it was instantiated by, and the
/// instance id carries it. Without this a descendant step reached every
/// node inside a widget *except* its rows — so `#grid > ~file-000.rs`
/// matched nothing however visible the row was, and the drives worked
/// around it with bare `~text` selectors that are safe only while the
/// text is unique in the window (M24/T24.4).
fn entry_is_inside(tree: &TreeRef, owner: usize, e: &Entry) -> bool {
    match e.idx {
        Some(j) => is_descendant(tree, owner, j),
        None => instance_of(e.id)
            .is_some_and(|(container, _)| container == owner || is_descendant(tree, owner, container)),
    }
}

fn is_descendant(tree: &TreeRef, i: usize, j: usize) -> bool {
    let mut k = j;
    while k < tree.table.len() {
        let p = tree.table[k].parent;
        if p == lumen_static::desc::NO_PARENT {
            return false;
        }
        if p as usize == i {
            return true;
        }
        k = p as usize;
    }
    false
}

fn resolve_one<'e>(ix: &'e Index<'_>, selector: &str) -> Result<&'e Entry, (i64, String, Value)> {
    // **`>` is a descendant step** — M27/T27.0's third landing.
    //
    // A part id lives on a *type's* table, so a composite instantiated
    // twice carries every id inside it twice and `W1011` reports both as
    // unaddressable. That had fired four times by the time this landed,
    // and the workaround each time was to split the type — which does
    // not scale and does not help an application that legitimately wants
    // two dropdowns.
    //
    // `#outer > #inner` says "the `#inner` **inside** `#outer`", so a
    // part id only has to be unique within its owner. The owner is
    // addressed the way it always was, by the `#[widget_id]` the
    // application gave *its* field, which is unique because the
    // application wrote it.
    //
    // `>` and not a space: a space already means "and", and changing
    // that would silently reinterpret every selector already written.
    if let Some((head, tail)) = selector.split_once('>') {
        // **Both sides, or refused** (M39/T39.6): an empty side used to
        // recurse into "empty selector", and the error was then garbled by
        // the substitution below.
        if head.trim().is_empty() || tail.trim().is_empty() {
            return Err((
                INVALID_PARAMS,
                format!("descendant step `>` needs a selector on both sides, in `{selector}`"),
                Value::Null,
            ));
        }
        let outer = resolve_one(ix, head.trim())
            .map_err(|(c, m, d)| (c, format!("in `{selector}`: {m}"), d))?;
        let Some(oi) = outer.idx else {
            return Err((
                INVALID_PARAMS,
                format!("`{}` names virtualized content, which has no subtree to \
                         search; address the row itself with `~text`", head.trim()),
                Value::Null,
            ));
        };
        let inner = Index {
            tree: ix.tree,
            entries: ix
                .entries
                .iter()
                .filter(|e| entry_is_inside(ix.tree, oi, e))
                .cloned()
                .collect(),
        };
        // Resolved against the subtree, so the error names the whole
        // selector **as a prefix** — not by `m.replace(tail, selector)`,
        // which put the selector in place of every occurrence of the tail
        // in the message: every character of it for an empty tail (a
        // 1,079 B request drew a 15,192 B reply), every `e` for `#x > e`
        // (M39/T39.6, R14-F6).
        return match resolve_one(&inner, tail.trim()) {
            Ok(e) => ix
                .entries
                .iter()
                .find(|x| x.id == e.id)
                .ok_or((NOT_FOUND, format!("selector `{selector}` matched nothing"), Value::Null)),
            Err((c, m, d)) => Err((c, format!("in `{selector}`: {m}"), d)),
        };
    }
    let terms = parse_selector(selector).map_err(|e| (INVALID_PARAMS, e, Value::Null))?;
    let matched: Vec<&Entry> =
        ix.entries.iter().filter(|e| terms.iter().all(|t| matches(ix.tree, e, t))).collect();
    match matched.len() {
        1 => Ok(matched[0]),
        0 => {
            // Name the term that killed the match, and what was available.
            for t in &terms {
                if !ix.entries.iter().any(|e| matches(ix.tree, e, t)) {
                    return Err((
                        NOT_FOUND,
                        format!("selector `{selector}` matched nothing"),
                        nearest(ix.tree, t),
                    ));
                }
            }
            Err((NOT_FOUND, format!("selector `{selector}` matched nothing"), Value::Null))
        }
        _ => Err((
            AMBIGUOUS,
            format!("selector `{selector}` matched {} nodes", matched.len()),
            json!({ "candidates": matched.iter().take(10).map(|e| ix.summary(e)).collect::<Vec<_>>() }),
        )),
    }
}

// ---- param helpers ----------------------------------------------------------

fn selector_param(params: &Value) -> Result<&str, (i64, String, Value)> {
    params.get("selector").and_then(Value::as_str).ok_or((
        INVALID_PARAMS,
        "missing `selector`".into(),
        Value::Null,
    ))
}

fn f64_param(params: &Value, key: &str) -> Result<f64, (i64, String, Value)> {
    params.get(key).and_then(Value::as_f64).ok_or((
        INVALID_PARAMS,
        format!("missing `{key}`"),
        Value::Null,
    ))
}
