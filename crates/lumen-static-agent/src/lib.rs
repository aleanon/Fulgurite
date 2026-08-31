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

use lumen_static::desc::{NodeDesc, NodeState, WidgetOps};
use lumen_static::observe::{self, accesskit};
use lumen_static::Widget;
use serde_json::{json, Map, Value};

mod audit;
mod journal;

use audit::Auditor;
use journal::{Coalesced, Journal};

/// A laid-out tree, borrowed for one dispatch.
#[derive(Clone, Copy)]
pub struct TreeRef<'a> {
    pub base: *const u8,
    pub table: &'a [NodeDesc],
    pub kinds: &'a [&'static WidgetOps],
    pub states: &'a [NodeState],
}

impl<'a> TreeRef<'a> {
    pub fn of_root<W: Widget>(root: &'a W, states: &'a [NodeState]) -> Self {
        TreeRef {
            base: root as *const W as *const u8,
            table: W::SUB_TABLE,
            kinds: W::KIND_LIST,
            states,
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
const METHOD_NOT_FOUND: i64 = -32601;

type RpcResult = Result<Value, (i64, String, Value)>;

impl Agent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed frame: the host calls this after layout +
    /// paint, and it is what makes `ui.lastChange`/`ui.lastDamage`
    /// answerable. Costs one extra paint-op dispatch per node (see
    /// `journal`); it is a dev surface, not a release path.
    pub fn frame_done(&mut self, tree: &TreeRef, layout_ns: u64, paint_ns: u64) {
        self.frame += 1;
        self.last_layout_ns = layout_ns;
        self.last_paint_ns = paint_ns;
        let structural_reset = self.journal.record(self.frame, tree);
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

    fn handle(&self, tree: &TreeRef, method: &str, params: &Value) -> RpcResult {
        match method {
            "ui.getTree" => Ok(observe::dump(tree.base, tree.table, tree.kinds, tree.states)),
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

    fn probe(&self, tree: &TreeRef, params: &Value) -> RpcResult {
        let (x, y) = (f64_param(params, "x")?, f64_param(params, "y")?);
        let ix = Index::build(tree);
        let hit = ix
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.contains(x, y))
            .max_by_key(|(pos, e)| (e.depth, *pos));
        match hit {
            Some((_, e)) => Ok(ix.summary(e)),
            None => Ok(Value::Null),
        }
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
    fn lint(&self, tree: &TreeRef) -> Value {
        let findings: Vec<Value> = self
            .auditor
            .current()
            .iter()
            .map(|f| {
                let mut o = self.node_ident(tree, f.idx);
                o.insert("code".into(), f.code.into());
                o.insert("message".into(), f.message.clone().into());
                o.insert("detail".into(), f.detail.clone());
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
            o.insert("kind".into(), tree.kinds[d.kind as usize].name.into());
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
                let mut o = Map::new();
                let i = c.idx as usize;
                let d = &tree.table[i];
                o.insert("index".into(), i.into());
                o.insert("kind".into(), tree.kinds[d.kind as usize].name.into());
                if !d.wid.is_empty() {
                    o.insert("wid".into(), d.wid.into());
                }
                if !d.src.file.is_empty() {
                    o.insert("src".into(), format!("{}:{}", d.src.file, d.src.line).into());
                }
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
        let repeaters: Vec<Value> = ix
            .entries
            .iter()
            .filter(|e| e.idx.is_some() && e.kind(tree) == Some("repeat"))
            .map(|e| {
                json!({
                    "index": e.idx.unwrap(),
                    "id": e.id,
                    "window": e.children.len(),
                    "rows": e.node.size_of_set(),
                })
            })
            .collect();
        json!({
            "frame": self.frame,
            "nodes": tree.table.len(),
            "total_nodes": ix.entries.len(),
            "kinds": tree.kinds.len(),
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

// ---- snapshot index ---------------------------------------------------------

struct Entry {
    id: u64,
    node: accesskit::Node,
    children: Vec<u64>,
    depth: u32,
    /// Main-table index, when this is not a synthesized instance node.
    idx: Option<usize>,
}

impl Entry {
    fn bounds(&self) -> Option<accesskit::Rect> {
        self.node.bounds()
    }
    fn contains(&self, x: f64, y: f64) -> bool {
        self.bounds().is_some_and(|b| x >= b.x0 && x < b.x1 && y >= b.y0 && y < b.y1)
    }
    fn intersects(&self, x: f64, y: f64, w: f64, h: f64) -> bool {
        self.bounds()
            .is_some_and(|b| b.x0 < x + w && b.x1 > x && b.y0 < y + h && b.y1 > y)
    }
    fn kind<'a>(&self, tree: &TreeRef<'a>) -> Option<&'static str> {
        self.idx.map(|i| tree.kinds[tree.table[i].kind as usize].name)
    }
}

struct Index<'a> {
    tree: &'a TreeRef<'a>,
    entries: Vec<Entry>,
}

impl<'a> Index<'a> {
    fn build(tree: &'a TreeRef<'a>) -> Self {
        let update = observe::snapshot(tree.base, tree.table, tree.kinds, tree.states);
        let mut entries: Vec<Entry> = update
            .nodes
            .into_iter()
            .map(|(nid, node)| {
                let children = node.children().to_vec().iter().map(|c| c.0).collect();
                let idx = if nid.0 & (1 << 63) == 0 {
                    Some((nid.0 - 1) as usize)
                } else {
                    None
                };
                Entry { id: nid.0, node, children, depth: 0, idx }
            })
            .collect();

        // Depths via BFS over children links from the root (id 1).
        let pos_of: std::collections::HashMap<u64, usize> =
            entries.iter().enumerate().map(|(p, e)| (e.id, p)).collect();
        let mut queue = std::collections::VecDeque::from([(1u64, 0u32)]);
        while let Some((id, depth)) = queue.pop_front() {
            let Some(&p) = pos_of.get(&id) else { continue };
            entries[p].depth = depth;
            for c in entries[p].children.clone() {
                queue.push_back((c, depth + 1));
            }
        }
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
            o.insert("kind".into(), self.tree.kinds[d.kind as usize].name.into());
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
                Term::Kind(k) => tree.kinds[d.kind as usize].name == *k,
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
            let mut names: Vec<&str> = tree.kinds.iter().map(|k| k.name).collect();
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
