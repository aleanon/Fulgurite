//! **M12/T12.1: a locator and expectation API.**
//!
//! A test should read like the thing it tests:
//!
//! ```ignore
//! let mut app = TestApp::new(counter(), (320, 200));
//! app.click("#inc")?;
//! app.expect_text("#value", "1")?;
//! ```
//!
//! Everything underneath already existed — `Agent::dispatch_input`, the
//! selector grammar, the one input queue — and every test that used it
//! assembled JSON-RPC by hand. That is fine for framework tests and
//! wrong for application tests, which is the audience this is for.
//!
//! # Three properties, each learned the hard way here
//!
//! 1. **Errors teach.** `ui.resolve` already answers a miss with the
//!    nearest candidates and an ambiguity with all of them. The harness
//!    surfaces that text rather than flattening it to "assertion
//!    failed" — an assertion that says *what it looked for, what it
//!    found, and what was nearby* is the difference between a two-minute
//!    fix and an afternoon.
//! 2. **Assertions state the semantic first.** State, semantics and
//!    bounds before pixels — and that is a property of the API rather
//!    than of the person using it: [`TestApp::expect_text`](crate::harness::TestApp::expect_text) is easier to
//!    reach for than a screenshot comparison, deliberately.
//! 3. **No auto-wait, and this says so.** There is no animation
//!    scheduler and `ui.waitSettled` is trivially true, so a retry loop
//!    here would be machinery pretending to solve a problem this
//!    architecture does not have. Every method below is synchronous: it
//!    pumps, and by the time it returns the frame is settled. If a
//!    scheduler ever arrives, waiting goes in with it.

use serde_json::{json, Value};

use crate::{Agent, AgentHost};

/// What went wrong, with enough context to fix it.
#[derive(Debug)]
pub struct Failure {
    /// What the harness was asked to do.
    pub what: String,
    /// What happened, in the protocol's own words — including the
    /// nearest candidates for a miss and every candidate for an
    /// ambiguity.
    pub why: String,
}

impl core::fmt::Display for Failure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.what, self.why)
    }
}

impl std::error::Error for Failure {}

/// The result of every harness call. `?` in a test body reads as
/// "and then", which is the whole point.
pub type Expect<T = ()> = Result<T, Failure>;

/// A driver over any [`AgentHost`] — which is every host this
/// repository has, headless or windowed.
pub struct TestApp<'h, H: AgentHost> {
    host: &'h mut H,
    agent: Agent,
    /// Every step taken, for the failure message. A test that fails on
    /// its ninth assertion is much easier to read with the first eight
    /// listed.
    steps: Vec<String>,
}

impl<'h, H: AgentHost> TestApp<'h, H> {
    /// Drive `host`. It should already have been pumped once, so the
    /// first selector has a laid-out tree to resolve against.
    pub fn new(host: &'h mut H) -> Self {
        TestApp { host, agent: Agent::new(), steps: Vec::new() }
    }

    /// The steps taken so far, oldest first.
    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    fn call(&mut self, method: &str, params: Value) -> Expect<Value> {
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let r = self.agent.dispatch_input(self.host, &req);
        if let Some(e) = r.get("error") {
            // **The protocol's own message, not a summary of it.** A
            // miss carries `nearest`; an ambiguity carries every
            // candidate. Flattening either to "not found" is what makes
            // a selector typo cost an afternoon.
            return Err(self.fail(
                format!("{method} {params}"),
                serde_json::to_string(e).unwrap_or_else(|_| "<unprintable>".into()),
            ));
        }
        Ok(r.get("result").cloned().unwrap_or(Value::Null))
    }

    fn fail(&self, what: String, why: String) -> Failure {
        let trail = if self.steps.is_empty() {
            String::new()
        } else {
            format!("\n  after: {}", self.steps.join(" -> "))
        };
        Failure { what, why: format!("{why}{trail}") }
    }

    /// Click the centre of whatever `selector` names.
    ///
    /// By author id through the protocol, so the path a real agent takes
    /// is the path the test takes.
    pub fn click(&mut self, selector: &str) -> Expect {
        let r = self.call("input.click", json!({ "selector": selector }))?;
        // **"Did that input land"** — the only direct answer, and the
        // only way to test an action whose correct outcome is that
        // nothing happens (M27/T27.10).
        if r.get("handled") == Some(&Value::Bool(false)) {
            return Err(self.fail(
                format!("click {selector}"),
                "the input reached no handler (`handled` was false)".into(),
            ));
        }
        self.steps.push(format!("click {selector}"));
        Ok(())
    }

    /// Type `text` into whatever `selector` names.
    pub fn fill(&mut self, selector: &str, text: &str) -> Expect {
        self.call("input.type", json!({ "selector": selector, "text": text }))?;
        self.steps.push(format!("fill {selector} {text:?}"));
        Ok(())
    }

    /// Press one key by name — `"Enter"`, `"Tab"`, `"Escape"`.
    pub fn press(&mut self, key: &str) -> Expect {
        self.call("input.key", json!({ "key": key }))?;
        self.steps.push(format!("press {key}"));
        Ok(())
    }

    /// The text a node shows: a `Label`'s content or a `Button`'s
    /// caption, whichever it has.
    pub fn text(&mut self, selector: &str) -> Expect<String> {
        let r = self.call("ui.resolve", json!({ "selector": selector }))?;
        // `value` for a Label, `label` for a Button. A harness that knew
        // only one of the two would send its user to the tree dump to
        // find out which — which is how this method earns its existence.
        for k in ["value", "label"] {
            if let Some(v) = r.get(k).and_then(Value::as_str) {
                return Ok(v.to_string());
            }
        }
        Err(self.fail(
            format!("text of {selector}"),
            format!("that node shows no text; it resolved to {r}"),
        ))
    }

    /// Assert the text a node shows.
    pub fn expect_text(&mut self, selector: &str, want: &str) -> Expect {
        let got = self.text(selector)?;
        if got != want {
            return Err(self.fail(
                format!("expect_text {selector}"),
                format!("wanted {want:?}, found {got:?}"),
            ));
        }
        self.steps.push(format!("expect_text {selector} == {want:?}"));
        Ok(())
    }

    /// A node's logical bounds, `[x0, y0, x1, y1]`.
    pub fn bounds(&mut self, selector: &str) -> Expect<[f32; 4]> {
        let r = self.call("ui.resolve", json!({ "selector": selector }))?;
        let b = r.get("bounds").and_then(Value::as_array).cloned().unwrap_or_default();
        if b.len() != 4 {
            return Err(self.fail(
                format!("bounds of {selector}"),
                format!("no bounds in {r}"),
            ));
        }
        let mut out = [0.0f32; 4];
        for (i, v) in b.iter().enumerate() {
            out[i] = v.as_f64().unwrap_or(0.0) as f32;
        }
        Ok(out)
    }

    /// Assert a node is on screen and has area.
    ///
    /// The bounds half of "state, semantics and bounds before pixels" —
    /// and the cheapest real check there is, because a control that
    /// resolves, reports a box, and sits outside the window is the
    /// failure `AGENT.md` records from M16/T16.7: a `Fixed(220)` card in
    /// a 300 px window put a button at x=256, every click hit the root,
    /// and the oracle compared two identical no-ops.
    pub fn expect_visible(&mut self, selector: &str, viewport: (f32, f32)) -> Expect {
        let b = self.bounds(selector)?;
        let (w, h) = (b[2] - b[0], b[3] - b[1]);
        if w <= 0.0 || h <= 0.0 {
            return Err(self.fail(
                format!("expect_visible {selector}"),
                format!("it has no area: {w} x {h}"),
            ));
        }
        if b[0] < 0.0 || b[1] < 0.0 || b[2] > viewport.0 || b[3] > viewport.1 {
            return Err(self.fail(
                format!("expect_visible {selector}"),
                format!(
                    "{b:?} is outside the {} x {} viewport",
                    viewport.0, viewport.1
                ),
            ));
        }
        self.steps.push(format!("expect_visible {selector}"));
        Ok(())
    }

    /// Assert that `selector` names nothing.
    ///
    /// The accepting direction for every locator above: without it a
    /// harness whose resolution silently matched the root would pass
    /// every `expect_text` against the wrong node.
    pub fn expect_absent(&mut self, selector: &str) -> Expect {
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "ui.resolve",
                         "params": {"selector": selector}});
        let r = self.agent.dispatch_input(self.host, &req);
        if r.get("error").is_none() {
            return Err(self.fail(
                format!("expect_absent {selector}"),
                format!("it resolved to {}", r.get("result").cloned().unwrap_or(Value::Null)),
            ));
        }
        self.steps.push(format!("expect_absent {selector}"));
        Ok(())
    }
}
