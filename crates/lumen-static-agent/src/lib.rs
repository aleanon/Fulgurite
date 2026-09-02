//! `lumen-static-agent` — the JSON-RPC 2.0 agent protocol (parity plan
//! P0/P1, `docs/plan-agent-parity.md`).
//!
//! Mirrors `lumen-agent`'s envelope and method vocabulary where the
//! semantics coincide, so agent tooling transfers. In-process dispatch
//! only for now (a WebSocket loop is a later, feature-gated shell
//! concern). Everything is read-side: queries over an already-laid-out
//! tree — input synthesis waits for the event system (P4).
//!
//! Selector grammar (space-separated terms, all must match):
//! - `#save`        author id (`#[widget_id("save")]`)
//! - `label`        widget kind name
//! - `~text`        value contains "text"
//! - `@app.rs:42`   source provenance (file suffix, optional line)
//! - `7`            table index
//!
//! Resolution errors teach (Lumen's ergonomic, adopted): not-found
//! carries the nearest misses, ambiguity carries the candidates.
//! Selectors resolve over main-table nodes; repeater instances are
//! reachable through `ui.probe`/`ui.getTree` (documented P1 limit).

use lumen_static::desc::{NodeDesc, NodeState};
use lumen_static::event::{Event, Key};
use lumen_static::hit::Hit;
use lumen_static::observe::{self, accesskit};
use lumen_static::widget::Node;
use lumen_static::{pass, Widget};
use serde_json::{json, Map, Value};

mod audit;
mod journal;

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
}

impl<'a> TreeRef<'a> {
    pub fn of_root<W: Widget>(
        root: &'a W,
        styles: &'a [lumen_static::Style],
        states: &'a [NodeState],
    ) -> Self {
        TreeRef { root, table: W::SUB_TABLE, states, styles }
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
}

impl<'a> TreeMut<'a> {
    pub fn of_root<W: Widget>(
        root: &'a mut W,
        styles: &'a [lumen_static::Style],
        states: &'a [NodeState],
    ) -> Self {
        TreeMut { root, table: W::SUB_TABLE, states, styles }
    }

    pub fn as_ref(&self) -> TreeRef<'_> {
        TreeRef {
            root: &*self.root,
            table: self.table,
            states: self.states,
            styles: self.styles,
        }
    }
}

/// Per-session agent state: frame counter, last pass timings, and the
/// change journal (P2), all fed by the host at each frame boundary.
#[derive(Default)]
pub struct Agent {
    pub frame: u64,
    pub last_layout_ns: u64,
    pub last_paint_ns: u64,
    journal: Journal,
    auditor: Auditor,
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
const METHOD_NOT_FOUND: i64 = -32601;

type RpcResult = Result<Value, (i64, String, Value)>;

impl Agent {
    pub fn new() -> Self {
        Self::default()
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
    pub fn frame_done(&mut self, tree: &mut TreeMut, layout_ns: u64, paint_ns: u64) {
        self.frame += 1;
        self.last_layout_ns = layout_ns;
        self.last_paint_ns = paint_ns;
        let structural_reset = self.journal.record(self.frame, &tree.as_ref());
        self.auditor.record(self.frame, tree, structural_reset);
    }

    /// Dispatch one JSON-RPC 2.0 request, returning the response object.
    pub fn dispatch(&self, tree: &TreeRef, req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
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
    pub fn dispatch_input(&self, host: &mut dyn Host, req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        if !method.starts_with("input.") {
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

    fn handle_input(&self, host: &mut dyn Host, method: &str, params: &Value) -> RpcResult {
        match method {
            "input.click" | "input.invokeAction" => {
                let (x, y) = self.point_param(host, params)?;
                host.enqueue(Event::PointerDown { x, y });
                host.enqueue(Event::PointerUp { x, y });
                Ok(json!({ "frame": host.pump(), "at": [x, y] }))
            }
            "input.hover" => {
                let (x, y) = self.point_param(host, params)?;
                host.enqueue(Event::PointerMove { x, y });
                Ok(json!({ "frame": host.pump(), "at": [x, y] }))
            }
            "input.scroll" => {
                let (x, y) = self.point_param(host, params)?;
                let dy = params.get("dy").and_then(Value::as_f64).unwrap_or(0.0) as f32;
                host.enqueue(Event::Scroll { x, y, dy });
                Ok(json!({ "frame": host.pump(), "at": [x, y], "dy": dy }))
            }
            "input.key" => {
                let name = params
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or((INVALID_PARAMS, "missing `key`".into(), Value::Null))?;
                let key = parse_key(name).ok_or_else(|| {
                    (INVALID_PARAMS, format!("unknown key `{name}`"), json!({ "known": KEY_NAMES }))
                })?;
                let down = params.get("down").and_then(Value::as_bool).unwrap_or(true);
                host.enqueue(Event::Key { key, down });
                Ok(json!({ "frame": host.pump() }))
            }
            "input.type" => {
                let text = params
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or((INVALID_PARAMS, "missing `text`".into(), Value::Null))?;
                // One JSON character becomes one queued Event, and the
                // queue has no cap and no backpressure — 2,000,000 chars
                // measured as 2,000,000 events in 51 ms. A real keyboard
                // has a rate limit; so does this. It is also the one
                // agent method whose *availability* effect a user with a
                // keyboard could not reproduce, which is the invariant
                // the input surface is otherwise held to
                // (`docs/plan-review-fixes.md` F2.6).
                let n = text.chars().count();
                if n > MAX_TYPE_CHARS {
                    return Err((
                        INVALID_PARAMS,
                        format!("`text` is {n} characters; the limit is {MAX_TYPE_CHARS}"),
                        json!({ "chars": n, "limit": MAX_TYPE_CHARS }),
                    ));
                }
                for ch in text.chars() {
                    host.enqueue(Event::Text { ch });
                }
                Ok(json!({ "frame": host.pump(), "chars": n }))
            }
            _ => Err((METHOD_NOT_FOUND, format!("unknown method `{method}`"), Value::Null)),
        }
    }

    /// `{x, y}` directly, or the centre of whatever `selector` resolves
    /// to — so agents can click by author id and never compute geometry.
    fn point_param(
        &self,
        host: &dyn Host,
        params: &Value,
    ) -> Result<(f32, f32), (i64, String, Value)> {
        if let (Some(x), Some(y)) = (
            params.get("x").and_then(Value::as_f64),
            params.get("y").and_then(Value::as_f64),
        ) {
            return Ok((x as f32, y as f32));
        }
        let tree = host.tree();
        let tree = &tree;
        let ix = Index::build(tree);
        let e = resolve_one(&ix, selector_param(params)?)?;
        let i = e.idx.ok_or((
            NOT_FOUND,
            "selector resolved to virtualized content; pass x/y instead".into(),
            Value::Null,
        ))?;
        let st = &tree.states[i];
        Ok((st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0))
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
            "ui.probe" => self.probe(tree, params),
            "ui.probeRegion" => self.probe_region(tree, params),
            "ui.lint" => Ok(self.lint(tree)),
            "app.logs" => Ok(self.logs(params)),
            "ui.lastChange" => Ok(self.last_change(tree, params)),
            "ui.lastDamage" => Ok(self.last_damage(params)),
            "app.diagnostics" => Ok(self.diagnostics(tree)),
            "ui.waitSettled" => {
                // No animation scheduler exists: a laid-out tree is settled.
                Ok(json!({ "settled": true, "frame": self.frame }))
            }
            "app.perf" => Ok(json!({
                "frame": self.frame,
                "layout_ns": self.last_layout_ns,
                "paint_ns": self.last_paint_ns,
            })),
            _ => Err((METHOD_NOT_FOUND, format!("unknown method `{method}`"), Value::Null)),
        }
    }

    /// `observe::dump` needs the concrete widget type for `SUB_TABLE`;
    /// through a `TreeRef` the table is carried alongside, so the JSON is
    /// assembled from the snapshot the same way.
    fn get_tree(&self, tree: &TreeRef) -> Value {
        let mut nodes = Vec::with_capacity(tree.table.len());
        observe::emit_table(tree.table, tree.root, tree.states, &|i| (i + 1) as u64, &mut nodes);
        let out: Vec<Value> = nodes
            .iter()
            .map(|(id, n)| {
                let mut o = Map::new();
                o.insert("id".into(), (id.0).into());
                o.insert("role".into(), format!("{:?}", n.role()).into());
                if let Some(v) = n.value() {
                    o.insert("value".into(), v.into());
                }
                if let Some(l) = n.label() {
                    o.insert("label".into(), l.into());
                }
                if let Some(b) = n.bounds() {
                    o.insert("bounds".into(), json!([b.x0, b.y0, b.x1, b.y1]));
                }
                let children: Vec<u64> = n.children().iter().map(|c| c.0).collect();
                if !children.is_empty() {
                    o.insert("children".into(), children.into());
                }
                let idx = (id.0 as usize).wrapping_sub(1);
                if id.0 & (1 << 63) == 0 && idx < tree.table.len() {
                    let d = &tree.table[idx];
                    if !d.src.file.is_empty() {
                        o.insert("src".into(), format!("{}:{}", d.src.file, d.src.line).into());
                    }
                    if !d.wid.is_empty() {
                        o.insert("wid".into(), d.wid.into());
                    }
                }
                Value::Object(o)
            })
            .collect();
        json!({ "root": 1, "nodes": out })
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
    fn probe(&self, tree: &TreeRef, params: &Value) -> RpcResult {
        let (x, y) = (f64_param(params, "x")? as f32, f64_param(params, "y")? as f32);
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
            // Mirrors Repeat::observe_op's encoding.
            Some(inn) => {
                let id = (1u64 << 63)
                    | ((h.node as u64 + 1) << 44)
                    | (inn.row << 12)
                    | inn.node as u64;
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
        let findings: Vec<Value> = self
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
            o.insert("kind".into(), tree.root.node(i).kind_name().into());
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
    fn since_param(&self, params: &Value) -> u64 {
        params
            .get("since")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| self.frame.saturating_sub(1))
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
    fn last_damage(&self, params: &Value) -> Value {
        let since = self.since_param(params);
        let report = self.journal.since(since);
        let rect = journal::damage(&report.changed);
        json!({
            "frame": self.frame,
            "since": since,
            "rect": rect.map(|r| json!([r[0], r[1], r[2], r[3]])).unwrap_or(Value::Null),
            "nodes": report.changed.len(),
            "truncated": report.truncated,
            "reset": report.reset,
        })
    }

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
        })
    }
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
pub trait Host {
    fn tree(&self) -> TreeRef<'_>;
    fn enqueue(&mut self, ev: Event);
    /// Drain the queue, re-lay out, record the frame; returns its number.
    fn pump(&mut self) -> u64;
}

const KEY_NAMES: [&str; 11] =
    ["Enter", "Escape", "Tab", "Backspace", "Delete", "Left", "Right", "Up", "Down", "Home", "End"];

fn parse_key(name: &str) -> Option<Key> {
    Some(match name {
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
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
        self.bounds()
            .is_some_and(|b| b.x0 < x + w && b.x1 > x && b.y0 < y + h && b.y1 > y)
    }
}

struct Index<'a> {
    tree: &'a TreeRef<'a>,
    entries: Vec<Entry>,
}

impl<'a> Index<'a> {
    fn build(tree: &'a TreeRef<'a>) -> Self {
        let mut nodes = Vec::with_capacity(tree.table.len());
        observe::emit_table(tree.table, tree.root, tree.states, &|i| (i + 1) as u64, &mut nodes);
        let entries: Vec<Entry> = nodes
            .into_iter()
            .map(|(nid, node)| {
                let idx = if nid.0 & (1 << 63) == 0 {
                    Some((nid.0 - 1) as usize)
                } else {
                    None
                };
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
        if let Some(b) = e.bounds() {
            o.insert("bounds".into(), json!([b.x0, b.y0, b.x1, b.y1]));
        }
        if let Some(i) = e.idx {
            let d = &self.tree.table[i];
            o.insert("index".into(), i.into());
            o.insert("kind".into(), self.tree.root.node(i).kind_name().into());
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
    ValueContains(&'s str),
    Src { file: &'s str, line: Option<u32> },
    TableIndex(usize),
}

fn parse_selector(sel: &str) -> Result<Vec<Term<'_>>, String> {
    let mut terms = Vec::new();
    for raw in sel.split_whitespace() {
        let term = if let Some(id) = raw.strip_prefix('#') {
            Term::Wid(id)
        } else if let Some(text) = raw.strip_prefix('~') {
            Term::ValueContains(text)
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
        Term::ValueContains(t) => e.node.value().is_some_and(|v| v.contains(t)),
        Term::TableIndex(i) => e.idx == Some(*i),
        Term::Wid(_) | Term::Kind(_) | Term::Src { .. } => {
            let Some(i) = e.idx else { return false };
            let d = &tree.table[i];
            match term {
                Term::Wid(w) => d.wid == *w,
                Term::Kind(k) => tree.root.node(i).kind_name() == *k,
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
            let mut names: Vec<&str> =
                (0..tree.table.len()).map(|i| tree.root.node(i).kind_name()).collect();
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
        Term::ValueContains(_) => Value::Null,
    }
}

fn resolve_one<'e>(
    ix: &'e Index<'_>,
    selector: &str,
) -> Result<&'e Entry, (i64, String, Value)> {
    let terms = parse_selector(selector).map_err(|e| (INVALID_PARAMS, e, Value::Null))?;
    let matched: Vec<&Entry> = ix
        .entries
        .iter()
        .filter(|e| terms.iter().all(|t| matches(ix.tree, e, t)))
        .collect();
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
    params
        .get("selector")
        .and_then(Value::as_str)
        .ok_or((INVALID_PARAMS, "missing `selector`".into(), Value::Null))
}

fn f64_param(params: &Value, key: &str) -> Result<f64, (i64, String, Value)> {
    params
        .get(key)
        .and_then(Value::as_f64)
        .ok_or((INVALID_PARAMS, format!("missing `{key}`"), Value::Null))
}
