//! **The agent's own panic is a reply; the application's is not
//! swallowed** — M39/T39.0 and T39.5 (R14-F5,
//! `docs/plan-agent-transport.md` §2.5).
//!
//! M36 guards the application code the agent reaches (`Live::pump`), but
//! `Agent::dispatch_input` itself ran bare inside `Session::agent`: its
//! selector parsing, `ui.getTree`, `ui.explain` and every widget's
//! `observe`. A panic there ended the window with status 101.
//!
//! No shipped widget panics in `observe`, and 1,250 hostile requests
//! found no real panic in the agent (R14-F5), so the fixture brings its
//! own: a leaf whose `observe` panics when armed, reached through
//! `ui.getTree`. The same leaf panics in `layout` on the other arm, which
//! is **application** code reached through `input.click` — and that one
//! must still unwind out of the session under `Recover::Off`, or a
//! shipped application would crash where no crash reporter sees it
//! (M36 rule 3).

use std::cell::Cell;

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{NodeDesc, NO_PARENT};
use lumen_static::event::{App, EventCtx, Handlers, NodeSel};
use lumen_static::observe::accesskit;
use lumen_static::prelude::*;
use lumen_static::style::{Seed, Style};
use lumen_static::{Color, Constraints, NodeState, Painter};
use lumen_static_shell::session::{FrameSource, Local, Recover, Screen, Session};
use lumen_static_shell::Host;
use serde_json::Value;

/// Nothing armed.
const SAFE: u8 = 0;
/// `observe` panics: the agent's own code path.
const IN_OBSERVE: u8 = 1;
/// `layout` panics: the application's.
const IN_LAYOUT: u8 = 2;

#[derive(Default)]
struct Bomb {
    arm: Cell<u8>,
}

impl Widget for Bomb {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "bomb";
    fn type_defaults() -> Style {
        Style::default()
    }
    const SEEDS: &'static [Seed] = &[Seed::new(
        Self::KIND,
        Self::type_defaults,
        lumen_static::style::Fields::of(&[lumen_static::style::Fields::VISIBLE]),
    )];
}

impl Node for Bomb {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        if self.arm.get() == IN_LAYOUT {
            panic!("the application's layout panicked on purpose (M39/T39.0)");
        }
        lumen_static::ops::record_painted(st, i, limits, Size { w: 40.0, h: 40.0 }, &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        out.rect(lumen_static::ops::rect_of(&st[i]), st[i].bg);
    }
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        if self.arm.get() == IN_OBSERVE {
            panic!("a widget's observe panicked on purpose (M39/T39.0)");
        }
        node.set_role(accesskit::Role::Label);
    }
    lumen_static::leaf_common!(Bomb, 0);
}

impl<S> Handlers<S> for Bomb {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("bomb")]
    bomb: Bomb,
}

#[derive(Default)]
struct Fx {
    ui: Ui,
}

impl App for Fx {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = Ui;
    type Msg = ();
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(_: &mut Self, _: ()) -> bool {
        false
    }
}

/// A screen that draws nothing: these tests read replies, not pixels.
struct Blind;

impl Screen for Blind {
    fn present(&mut self, _src: &mut dyn FrameSource, _page: Color) {}
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        None
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

fn session(recover: Recover) -> Local<Fx> {
    let mut host: Host<Fx, _, _> = Host::new(Fx::default(), 200, 200);
    host.pump();
    Local::from_host(host, "agent-panic", Color(255, 255, 255, 255), recover)
}

fn ask(s: &mut Local<Fx>, method: &str, params: &str) -> Value {
    let req = format!(r#"{{"jsonrpc":"2.0","id":7,"method":"{method}","params":{params}}}"#);
    serde_json::from_str(&s.agent(&req, &mut Blind)).expect("the reply is JSON")
}

/// The agent's own panic becomes `-32603`, naming the method and where
/// it happened, and the next call is answered — under both `Recover`
/// values, because the agent is not application code (§2.5).
#[test]
#[ignore = "red until T39.5: R14-F5, the agent's own panic ends the process"]
fn the_agents_own_panic_is_a_reply() {
    for recover in [Recover::On, Recover::Off] {
        let mut s = session(recover);
        let before = ask(&mut s, "ui.getTree", "{}");
        assert!(before.get("result").is_some(), "{recover:?}: an ordinary call first: {before}");

        s.host.app.ui.bomb.arm.set(IN_OBSERVE);
        let reply = ask(&mut s, "ui.getTree", "{}");
        s.host.app.ui.bomb.arm.set(SAFE);
        let after = ask(&mut s, "ui.getTree", "{}");

        let code = reply["error"]["code"].as_i64();
        let msg = reply["error"]["message"].as_str().unwrap_or("");
        let file = reply["error"]["data"]["location"]["file"].as_str().unwrap_or("");
        println!(
            "{recover:?}: reply code {code:?} / expected -32603; location {file:?}; \
             next call ok: {}",
            after.get("result").is_some()
        );
        assert_eq!(code, Some(-32603), "{recover:?}: {reply}");
        assert!(msg.contains("ui.getTree"), "{recover:?}: the reply names no method: {reply}");
        assert!(file.ends_with("agent_panic.rs"), "{recover:?}: the reply names no location: {reply}");
        assert_eq!(reply["id"], 7, "{recover:?}: the id is not echoed: {reply}");
        assert!(after.get("result").is_some(), "{recover:?}: the next call was refused: {after}");
    }
}

/// **The accept direction**: an application panic reached through
/// `input.click` under `Recover::Off` still unwinds out of the session.
/// Green before T39.5 and after it; T39.5's second injection (drop the
/// re-raise) is what turns it red.
#[test]
fn an_application_panic_still_unwinds_under_recover_off() {
    let mut s = session(Recover::Off);
    s.host.app.ui.bomb.arm.set(IN_LAYOUT);
    s.host.invalidate_all();
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ask(&mut s, "input.click", r#"{"x":10,"y":10}"#)
    }));
    match out {
        Err(p) => {
            let m = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            println!("Recover::Off: the application's panic unwound out of the session: {m:?}");
            assert!(m.contains("layout panicked"), "a different panic unwound: {m:?}");
        }
        Ok(reply) => panic!("the application's panic was swallowed into a reply: {reply}"),
    }
}

/// And under `Recover::On` the pump's own guard owns it: a result, and
/// `app.failure` reports it (M36) — the outer catch never sees it.
#[test]
fn an_application_panic_under_recover_on_is_m36s() {
    let mut s = session(Recover::On);
    s.host.app.ui.bomb.arm.set(IN_LAYOUT);
    s.host.invalidate_all();
    let reply = ask(&mut s, "input.click", r#"{"x":10,"y":10}"#);
    assert!(reply.get("result").is_some(), "{reply}");
    let failure = ask(&mut s, "app.failure", "{}");
    let m = failure["result"]["failure"]["message"].as_str().unwrap_or("");
    println!("Recover::On: click answered, app.failure says {m:?}");
    assert!(m.contains("layout panicked"), "{failure}");
}
