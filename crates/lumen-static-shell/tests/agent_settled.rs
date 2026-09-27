//! **`ui.waitSettled` answers from the host** — M48/T48.0 and T48.2
//! (R13-F1, `docs/plan-agent-truth.md` §2.2).
//!
//! It was `Ok({"settled": true})` under a comment saying no animation
//! scheduler existed, which stopped being true when `App::tick` and the
//! task inbox landed. Two cases, each carrying one term of the answer:
//! an animation that runs for 200 ms, and a job that sleeps 100 ms on the
//! window's own `Threads` spawner. Each is asked while it is moving, and
//! again once it has stopped — the "after" is the accept direction.

use std::time::{Duration, Instant};

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, JobFn, Outbox, Subs};
use lumen_static::Color;
use lumen_static_shell::session::{FrameSource, Local, Crashing, Screen, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use serde_json::Value;

const KIND_SLOW: u64 = 1;
/// A subscription whose job runs for longer than the test.
const KIND_FEED: u64 = 2;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("status")]
    status: Label<String>,
    #[widget_id("animate")]
    animate: Button<Fx>,
    #[widget_id("slow")]
    slow: Button<Fx>,
}

#[derive(Default)]
struct Fx {
    ui: Ui,
    outbox: Outbox,
    /// Milliseconds of animation left.
    anim_ms: f32,
    subs: Subs,
    landed: u64,
}

fn feed(_d: Descriptor, _tx: &lumen_static::inbox::Sender<u64>) {
    std::thread::sleep(Duration::from_secs(30));
}

fn slow(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    std::thread::sleep(Duration::from_millis(100));
    let _ = tx.send(d.payload);
}

impl App for Fx {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(this: &mut Self, m: u64) -> bool {
        this.landed = m;
        this.ui.status.set(format!("slow={m}"));
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        match d.kind {
            KIND_SLOW => Some(slow as JobFn<u64>),
            KIND_FEED => Some(feed as JobFn<u64>),
            _ => None,
        }
    }
    fn subscriptions(&mut self) -> Option<&mut Subs> {
        Some(&mut self.subs)
    }
    fn tick(&mut self, dt_ms: f32) -> bool {
        self.anim_ms = (self.anim_ms - dt_ms).max(0.0);
        self.anim_ms > 0.0
    }
}

struct Blind;

impl Screen for Blind {
    fn present(&mut self, _src: &mut dyn FrameSource, _page: Color) -> lumen_static_shell::session::FrameTimes {
        Default::default()
    }
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        None
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

fn session() -> Local<Fx, Crashing> {
    let mut app = Fx::default();
    app.ui.animate.on_click = |f: &mut Fx, _: &lumen_static::EventCtx| f.anim_ms = 200.0;
    app.ui.slow.on_click = |f: &mut Fx, _: &lumen_static::EventCtx| {
        f.outbox.push(Descriptor::new(KIND_SLOW, 99));
    };
    let host: Host<Fx, _, _> = Host::new(app, 300, 200);
    // `from_host` installs the window's own spawner, `Threads`.
    let mut s = Local::from_host(host, "agent-settled", Color(255, 255, 255, 255), Crashing);
    s.frame(0.0, 300, 200);
    s
}

fn ask(s: &mut Local<Fx, Crashing>, method: &str, params: &str) -> Value {
    let req = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let r: Value = serde_json::from_str(&s.agent(&req, &mut Blind)).expect("the reply is JSON");
    assert!(r.get("error").is_none(), "{method} failed: {r}");
    r["result"].clone()
}

fn settled(s: &mut Local<Fx, Crashing>, when: &str) -> Value {
    let r = ask(s, "ui.waitSettled", "{}");
    println!(
        "{when}: settled={} animating={} pending={} more_deferred={} deadline={}",
        r["settled"], r["animating"], r["pending_tasks"], r["more_deferred"], r["next_deadline_ms"]
    );
    r
}

#[test]
fn an_animation_is_not_settled_until_it_stops() {
    let mut s = session();
    assert_eq!(settled(&mut s, "still")["settled"], true, "a still window must answer settled");
    let r = ask(&mut s, "input.click", r##"{"selector":"#animate"}"##);
    assert_eq!(r["handled"], true, "setup: the click did not land: {r}");
    // The redraw the RPC asks for: tick, pump, and the zero-dt query that
    // says whether anything is still moving.
    let animating = s.frame(16.0, 300, 200);
    assert!(animating, "setup: the frame after the click is not animating");
    let during = settled(&mut s, "animating");
    assert_eq!(during["settled"], false, "settled while animating: {during}");
    assert_eq!(during["animating"], true, "the reply does not say why: {during}");
    let mut frames = 0;
    while s.frame(50.0, 300, 200) {
        frames += 1;
        assert!(frames < 20, "setup: the animation never ended");
    }
    let after = settled(&mut s, "stopped");
    assert_eq!(after["settled"], true, "still unsettled after the animation ended: {after}");
}

#[test]
fn a_task_in_flight_is_not_settled_until_its_result_is_applied() {
    let mut s = session();
    let r = ask(&mut s, "input.click", r##"{"selector":"#slow"}"##);
    assert_eq!(r["handled"], true, "setup: the click did not land: {r}");
    s.frame(0.0, 300, 200);
    let during = settled(&mut s, "task sleeping");
    assert_eq!(s.host.app.landed, 0, "setup: the job finished before it was asked about");
    assert_eq!(during["settled"], false, "settled with a job in flight: {during}");
    assert_eq!(during["pending_tasks"], 1, "the reply does not count the job: {during}");
    let t0 = Instant::now();
    while s.host.app.landed == 0 {
        assert!(t0.elapsed() < Duration::from_secs(5), "setup: the job's result never arrived");
        std::thread::sleep(Duration::from_millis(10));
        s.frame(10.0, 300, 200);
    }
    let after = settled(&mut s, "result applied");
    assert_eq!(after["settled"], true, "still unsettled after the result was applied: {after}");
    assert_eq!(after["pending_tasks"], 0, "{after}");
}

/// **A subscription is counted, not waited on** (plan §2.2): its job runs
/// for as long as it is declared, and waiting on it would keep a
/// subscribed application from ever settling. M47/T47.3's ledger lists it
/// the whole time, which is why the count here is the ledger less the
/// running subscriptions.
#[test]
fn a_running_subscription_does_not_keep_a_window_unsettled() {
    let mut s = session();
    s.host.app.subs.declare(KIND_FEED, 7);
    s.frame(0.0, 300, 200);
    let r = settled(&mut s, "subscribed");
    println!("  in flight (the whole ledger) {}", s.host.in_flight());
    assert_eq!(s.host.in_flight(), 1, "setup: the subscription's job is not running");
    assert_eq!(r["subscriptions"], 1, "the reply does not count the subscription: {r}");
    assert_eq!(r["settled"], true, "a running subscription kept the window unsettled: {r}");
}
