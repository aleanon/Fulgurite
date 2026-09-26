//! **A reply's `frame` is one `ui.lastChange` can find** — M48/T48.0 and
//! T48.1 (R13-F3, `docs/plan-agent-truth.md` §2.1).
//!
//! An `input.*` reply took its `frame` from `Live::pump`, which returns
//! the host's frame counter; `ui.lastChange` and `ui.lastDamage` count the
//! agent's, which advances once per `frame_done`. The host's moves twice
//! per input RPC in a window — once for the pump inside the RPC and once
//! for the redraw after it — so `since = reply.frame - 1` drifted past
//! every change the click made.
//!
//! The session is driven the way the window drives it: the RPC, then
//! `Session::frame` (the redraw `answer_agent` requests), then
//! `agent_frame_done`. **Five clicks**, because in a fresh session the two
//! counters may agree for the first one.

use lumen_static::prelude::*;
use lumen_static::Color;
use lumen_static_shell::session::{FrameSource, Local, Recover, Screen, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use serde_json::Value;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("count")]
    count: Label<String>,
    #[widget_id("inc")]
    inc: Button<Tally>,
}

#[derive(DeriveApp, Default)]
struct Tally {
    #[tree]
    ui: Ui,
    #[bind(ui.count, "count {}")]
    n: i64,
}

/// A screen that draws nothing: this test reads replies, not pixels.
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

fn ask(s: &mut Local<Tally>, method: &str, params: &str) -> Value {
    let req = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let r: Value = serde_json::from_str(&s.agent(&req, &mut Blind)).expect("the reply is JSON");
    assert!(r.get("error").is_none(), "{method} failed: {r}");
    r["result"].clone()
}

/// The redraw `answer_agent` asks for, as `Win::redraw` runs it.
fn redraw(s: &mut Local<Tally>) {
    s.frame(16.0, 300, 200);
    s.agent_frame_done();
}

#[test]
fn a_replys_frame_finds_its_change_on_every_click() {
    let mut app = Tally::default();
    app.ui.inc.on_click = |t: &mut Tally, _: &lumen_static::EventCtx| t.n += 1;
    let host: Host<Tally, _, _> = Host::new(app, 300, 200);
    let mut s = Local::from_host(host, "agent-frames", Color(255, 255, 255, 255), Recover::Off);
    // The window's first redraw, which seeds the journal.
    redraw(&mut s);

    let count = ask(&mut s, "ui.resolve", r##"{"selector":"#count"}"##)["index"].as_u64().expect("#count has an index");
    let mut bad = Vec::new();
    for k in 1..=5 {
        let r = ask(&mut s, "input.click", r##"{"selector":"#inc"}"##);
        assert_eq!(r["handled"], true, "setup: click {k} did not land: {r}");
        let f = r["frame"].as_u64().expect("a reply names a frame");
        redraw(&mut s);
        let lc = ask(&mut s, "ui.lastChange", &format!(r#"{{"since":{}}}"#, f.saturating_sub(1)));
        let changed = lc["changed"].as_array().cloned().unwrap_or_default();
        let has_count = changed.iter().any(|c| c["index"].as_u64() == Some(count));
        assert_eq!(s.host.app.n, k, "setup: the handler ran {} times after {k} clicks", s.host.app.n);
        println!(
            "click {k}: reply frame {f}, changes {} (must be >= 1), #count among them: {has_count}; \
             journal frame {}",
            changed.len(),
            lc["frame"]
        );
        if !has_count {
            bad.push(k);
        }
    }
    assert!(bad.is_empty(), "clicks {bad:?}: `since = reply.frame - 1` did not find the changed label");
}
