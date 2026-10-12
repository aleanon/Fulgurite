//! The protocol's *domain*, not its vocabulary.
//!
//! Every other agent test supplies parameters the implementation would
//! produce: `since` takes only 0, 1, and `frame - 1`. Two `since + 1`
//! expressions therefore ran on nothing but small integers, and both
//! panicked in debug on `u64::MAX` — one of them reachable with a single
//! recorded frame, the other only once the 256-entry log ring has
//! dropped something, which is exactly the long session an agent
//! attaches to. `docs/plan-review-fixes.md` F1.3.
//!
//! An agent is untrusted input. Everything here asserts the host stays
//! up and answers, never that it computes a particular number.
//!
//! **That is true of the socket as well as of dispatch since M39.** The
//! transport (`fulgurite_agent::transport`) ends a connection on any
//! line that is not a JSON object, so a web page's HTTP request no longer
//! reaches `dispatch` at all; its gates are `tests/transport.rs`. What
//! reaches this file is a well-framed request, which is still untrusted.
//! And a live session answers a panic *in the agent* with `-32603`
//! rather than dying (M39/T39.5,
//! `crates/fulgurite-shell/tests/agent_panic.rs`).

use fulgurite_widgets::ops::{Fixed, Label, Spacer};
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
use fulgurite_agent::{Agent, TreeMut, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Bar {
    #[widget_id("title")]
    title: Label,
    pad: Spacer,
    #[widget_id("badge")]
    badge: Fixed,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

struct Fixture {
    app: Bar,
    states: [NodeState; Bar::NODE_COUNT],
    styles: Vec<fulgurite::Style>,
    agent: Agent,
}

impl Fixture {
    fn new() -> Self {
        let app = Bar::default();
        let styles = pass::default_styles(&app);
        let mut f = Fixture {
            app,
            states: [NodeState::default(); Bar::NODE_COUNT],
            styles,
            agent: Agent::new(),
        };
        f.frame();
        f
    }

    fn frame(&mut self) {
        pass::layout(&mut self.app, &self.styles, &mut self.states, VIEW);
        let mut tree = TreeMut::of_root(&mut self.app, &self.styles, &self.states);
        self.agent.frame_done(&mut tree, fulgurite_agent::FrameTiming::passes(1, 1));
    }

    /// Churn enough frames that the audit's log ring drops entries —
    /// the precondition for reaching `audit.rs`'s `since + 1`.
    fn churn(&mut self, frames: usize) {
        for i in 0..frames {
            // Alternate a lint on and off so transitions keep pushing.
            self.app.badge.w = if i % 2 == 0 { 0.0 } else { 40.0 };
            self.app.badge.h = if i % 2 == 0 { 0.0 } else { 10.0 };
            self.frame();
        }
    }

    fn raw(&self, req: Value) -> Value {
        let tree = TreeRef::of_root(&self.app, &self.styles, &self.states);
        self.agent.dispatch(&tree, &req)
    }
}

/// Every value that has ever been mistaken for a frame number.
fn hostile_since() -> Vec<Value> {
    vec![
        json!(u64::MAX),
        json!(u64::MAX - 1),
        json!(i64::MAX),
        json!(-1),
        json!(1.5),
        json!("abc"),
        json!(null),
        json!([]),
        json!({}),
    ]
}

const SINCE_METHODS: [&str; 3] = ["ui.lastChange", "ui.lastDamage", "app.logs"];

/// A response is well-formed if it is a JSON-RPC envelope carrying
/// exactly one of `result` or `error`. Nothing here cares which.
fn assert_answered(resp: &Value, what: &str) {
    assert_eq!(resp["jsonrpc"], "2.0", "{what}: not an envelope: {resp}");
    let has_result = resp.get("result").is_some();
    let has_error = resp.get("error").is_some();
    assert!(has_result ^ has_error, "{what}: must carry exactly one of result/error: {resp}");
}

#[test]
fn a_hostile_since_never_takes_the_host_down() {
    let f = Fixture::new();
    for m in SINCE_METHODS {
        for s in hostile_since() {
            let resp = f.raw(json!({
                "jsonrpc": "2.0", "id": 1, "method": m, "params": { "since": s }
            }));
            assert_answered(&resp, &format!("{m} since={s}"));
        }
    }
}

#[test]
fn a_hostile_since_survives_a_dropped_log_ring() {
    // `audit.rs`'s `since + 1` sits behind `self.dropped > 0`, so it is
    // unreachable until the 256-entry ring overflows. Every agent test
    // before this one stopped well short.
    let mut f = Fixture::new();
    f.churn(400);
    for m in SINCE_METHODS {
        for s in hostile_since() {
            let resp = f.raw(json!({
                "jsonrpc": "2.0", "id": 1, "method": m, "params": { "since": s }
            }));
            assert_answered(&resp, &format!("churned {m} since={s}"));
        }
    }
}

#[test]
fn malformed_envelopes_are_answered_not_panicked() {
    let f = Fixture::new();
    let reqs = vec![
        json!({}),
        json!({ "jsonrpc": "2.0" }),
        json!({ "jsonrpc": "2.0", "id": 1 }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "" }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "no.such.method" }),
        json!({ "jsonrpc": "2.0", "id": 1.5, "method": "ui.lastChange" }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": "not an object" }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": [] }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": 42 }),
        json!("not an object at all"),
        json!([1, 2, 3]),
    ];
    for r in reqs {
        let resp = f.raw(r.clone());
        assert_eq!(resp["jsonrpc"], "2.0", "not an envelope for {r}: {resp}");
    }
}

#[test]
fn hostile_selectors_are_answered_not_panicked() {
    let f = Fixture::new();
    for sel in [
        json!(""),
        json!("   "),
        json!("~"),
        json!("#"),
        json!("@:"),
        json!("@a:99999999999999999999"),
        json!("99999999999999999999999999"),
        json!("#\u{0}"),
        json!(json!({ "not": "a string" })),
        json!("#".repeat(4096)),
    ] {
        for m in ["ui.resolve", "ui.getLayout"] {
            let resp = f.raw(json!({
                "jsonrpc": "2.0", "id": 1, "method": m, "params": { "selector": sel }
            }));
            assert_eq!(resp["jsonrpc"], "2.0", "{m} {sel}: {resp}");
        }
    }
}

/// `input.*` needs a host, so it goes through `dispatch_input` with a
/// minimal one rather than the read-only fixture above.
mod input_limits {
    use super::*;
    use fulgurite::{Event, InputQueue};
    use fulgurite_agent::AgentHost;

    struct Bare {
        app: Bar,
        states: [NodeState; Bar::NODE_COUNT],
        styles: Vec<fulgurite::Style>,
        queue: InputQueue,
        frame: u64,
    }

    impl AgentHost for Bare {
        fn tree(&self) -> TreeRef<'_> {
            TreeRef::of_root(&self.app, &self.styles, &self.states)
        }
        fn enqueue(&mut self, ev: Event) {
            self.queue.push(ev);
        }
        fn pump(&mut self) -> (u64, bool) {
            while self.queue.pop().is_some() {}
            self.frame += 1;
            (self.frame, false)
        }
    }

    fn bare() -> Bare {
        let app = Bar::default();
        let styles = pass::default_styles(&app);
        Bare {
            app,
            states: [NodeState::default(); Bar::NODE_COUNT],
            styles,
            queue: InputQueue::default(),
            frame: 0,
        }
    }

    #[test]
    fn input_type_refuses_an_unbounded_string() {
        let agent = Agent::new();
        let mut h = bare();

        let ok = agent.dispatch_input(
            &mut h,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "input.type",
                     "params": { "text": "hello" } }),
        );
        assert_eq!(ok["result"]["chars"], 5, "ordinary typing is unaffected");

        // One JSON character was one queued event, with no cap: 2M chars
        // measured as 2M events. A keyboard cannot do that.
        let huge = "x".repeat(100_000);
        let resp = agent.dispatch_input(
            &mut h,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "input.type",
                     "params": { "text": huge } }),
        );
        assert_eq!(resp["error"]["code"], -32602, "refused: {resp}");
        assert_eq!(resp["error"]["data"]["chars"], 100_000);
    }

    /// **The envelope is validated, and a coordinate must be finite** —
    /// M39/T39.0 (R14-F7).
    ///
    /// `dispatch` took the method as `as_str().unwrap_or("")`, so a
    /// batch, a bare number or a non-string method fell through to
    /// "unknown method ``", a request with no `id` was answered, and
    /// `jsonrpc: "1.0"` was accepted. Four sites cast a coordinate
    /// `as f32` with no finiteness check, so `x: 1e300` was a success
    /// answered `at: [null, null]`. Every row prints `input -> code
    /// (expected)`; the last row is the accept direction.
    #[test]
    fn the_envelope_is_validated() {
        let agent = Agent::new();
        let mut h = bare();
        let click = json!({ "jsonrpc": "2.0", "id": 1, "method": "input.click",
                            "params": { "x": 1.0, "y": 1.0 } });
        let rows: Vec<(Value, Option<i64>)> = vec![
            (json!([]), Some(-32600)),
            (json!([click.clone()]), Some(-32600)),
            (json!(5), Some(-32600)),
            (json!({ "jsonrpc": "2.0", "id": 1, "method": 5 }), Some(-32600)),
            (json!({ "jsonrpc": "1.0", "id": 1, "method": "ui.lastChange" }), Some(-32600)),
            (json!({ "jsonrpc": "2.0", "method": "input.click", "params": { "x": 1.0, "y": 1.0 } }),
             Some(-32600)),
            (json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": "x" }),
             Some(-32600)),
            (json!({ "jsonrpc": "2.0", "id": 1, "method": "input.click",
                     "params": { "x": 1e300, "y": 0.0 } }), Some(-32602)),
            (json!({ "jsonrpc": "2.0", "id": 1, "method": "input.scroll",
                     "params": { "x": 1.0, "y": 1.0, "dy": 1e39 } }), Some(-32602)),
            (json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.probe",
                     "params": { "x": 1e300, "y": 0.0 } }), Some(-32602)),
            (json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.explain",
                     "params": { "selector": "#title", "at": [1e300, 0.0] } }), Some(-32602)),
            // The accept row: a well-formed click is still a result.
            (click, None),
        ];
        let mut failed = Vec::new();
        for (req, want) in rows {
            let resp = agent.dispatch_input(&mut h, &req);
            let got = resp["error"]["code"].as_i64();
            let shown = |c: Option<i64>| c.map_or("result".to_string(), |c| c.to_string());
            println!("{req} -> {} (expected {})", shown(got), shown(want));
            if got != want {
                failed.push(format!("{req}: got {} want {} ({resp})", shown(got), shown(want)));
            }
        }
        assert!(failed.is_empty(), "the envelope is not validated:\n{}", failed.join("\n"));
    }
}
