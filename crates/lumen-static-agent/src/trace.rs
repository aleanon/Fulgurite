//! What to write down when a test fails — plan M12/T12.4.
//!
//! Every fact here is already an agent method: `ui.getTree`,
//! `ui.lastChange`, `ui.lastDamage`, `app.perf`, `ui.lint`,
//! `ui.screenshot`. What was missing is writing them out **at the
//! moment of failure**, instead of leaving the reader to re-run and
//! re-query — by which time the frame is gone.
//!
//! # The one addition over Lumen's trace
//!
//! **`ui.explain` on the selector that failed.** "I clicked `#save` and
//! nothing happened" is the commonest failure a UI test has, and this
//! protocol already answers it by running the *real* hit test rather
//! than describing the tree. A trace that carries the answer turns a
//! re-run into a read.
//!
//! # Deliberately not a log of everything
//!
//! A trace is written on failure, not per frame. A per-frame trace of a
//! passing run is a file nobody opens, and this crate's own audit is
//! built on the opposite principle — the ambient layer reports
//! *findings*, not activity.

use serde_json::{json, Value};

/// A failure, with everything the frame knew about it.
pub struct Trace {
    pub test: String,
    pub message: String,
    entries: Vec<(String, Value)>,
}

impl Trace {
    pub fn new(test: &str, message: &str) -> Self {
        Trace { test: test.into(), message: message.into(), entries: Vec::new() }
    }

    /// Ask the agent something and keep the answer.
    ///
    /// Errors are kept too, and as the error: a method that failed is
    /// itself a fact about the frame, and swallowing it would leave a
    /// gap exactly where the reader is looking.
    pub fn ask(
        &mut self,
        agent: &crate::Agent,
        tree: &crate::TreeRef,
        method: &str,
        params: Value,
    ) -> &mut Self {
        let resp = agent.dispatch(
            tree,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
        );
        let value = resp
            .get("result")
            .cloned()
            .unwrap_or_else(|| resp.get("error").cloned().unwrap_or(Value::Null));
        self.entries.push((method.to_owned(), value));
        self
    }

    /// The standard set: what changed, what is wrong, and — when a
    /// selector is named — **why a click on it would or would not
    /// land**.
    pub fn collect(
        &mut self,
        agent: &crate::Agent,
        tree: &crate::TreeRef,
        selector: Option<&str>,
    ) -> &mut Self {
        if let Some(sel) = selector {
            // First, because it is the answer most failures want.
            self.ask(agent, tree, "ui.explain", json!({ "selector": sel }));
            self.ask(agent, tree, "ui.getAppliedStyles", json!({ "selector": sel }));
        }
        self.ask(agent, tree, "ui.lint", json!({}));
        self.ask(agent, tree, "ui.lastChange", json!({}));
        self.ask(agent, tree, "ui.lastDamage", json!({}));
        self.ask(agent, tree, "app.diagnostics", json!({}));
        self.ask(agent, tree, "ui.getTree", json!({}))
    }

    /// Newline-delimited JSON — Lumen's format, so its tooling reads
    /// ours.
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        out.push_str(&json!({ "test": self.test, "failure": self.message }).to_string());
        out.push('\n');
        for (method, value) in &self.entries {
            out.push_str(&json!({ "method": method, "result": value }).to_string());
            out.push('\n');
        }
        out
    }

    /// A short human summary — the part a reader sees first, and often
    /// all they need.
    pub fn summary(&self) -> String {
        let mut out = format!("{}: {}\n", self.test, self.message);
        for (method, value) in &self.entries {
            if method == "ui.explain" {
                if let Some(s) = value.get("summary").and_then(Value::as_str) {
                    out.push_str(&format!("  explain: {s}\n"));
                }
                for r in value.get("reasons").and_then(Value::as_array).into_iter().flatten() {
                    if let Some(d) = r.get("detail").and_then(Value::as_str) {
                        out.push_str(&format!("    - {d}\n"));
                    }
                }
            }
            if method == "ui.lint" {
                let n = value.get("findings").and_then(Value::as_array).map_or(0, |a| a.len());
                if n > 0 {
                    out.push_str(&format!("  lint: {n} finding(s)\n"));
                }
            }
        }
        out
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
