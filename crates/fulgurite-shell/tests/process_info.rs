//! **A host names the strategy it runs, and a session reports it** —
//! M42/T42.1 (`docs/plan-drive-identity.md` §2.1, R18-F1).
//!
//! The drive harness used to take a leg's strategy from a label the
//! script set for itself (an environment variable), so a leg built with the
//! wrong features — or a leftover process on the port — was driven and
//! reported under whatever the script believed. The strategy now comes
//! from the process: `Host::strategy` is `type_name` of the host's own
//! invalidation parameter, and `app.diagnostics` carries it as
//! `process.strategy`.

use fulgurite::invalidate::{Boundaries, Everything, Invalidation};
use fulgurite::prelude::*;
use fulgurite::retain::Incremental;
use fulgurite::Color;
use fulgurite_shell::session::{Crashing, DefaultInval, FrameSource, Local, Screen, Session};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;
use serde_json::Value;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("value")]
    value: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Fx {
    #[tree]
    ui: Ui,
}

fn strategy_of<I: Invalidation>() -> &'static str {
    let h: Host<Fx, Incremental, I> = Host::new(Fx::default(), 100, 40);
    h.strategy()
}

#[test]
fn an_everything_host_names_everything() {
    let s = strategy_of::<Everything>();
    println!("Host<_, _, Everything>::strategy() = {s:?}");
    assert!(s.ends_with("Everything"), "expected ...Everything, got {s:?}");
}

#[test]
fn a_boundaries_host_names_boundaries() {
    let s = strategy_of::<Boundaries<256>>();
    println!("Host<_, _, Boundaries<256>>::strategy() = {s:?}");
    assert!(s.ends_with("Boundaries<256>"), "expected ...Boundaries<256>, got {s:?}");
}

/// A screen that draws nothing: this test reads replies, not pixels.
struct Blind;

impl Screen for Blind {
    fn present(&mut self, _src: &mut dyn FrameSource, _page: Color) -> fulgurite_shell::session::FrameTimes {
        Default::default()
    }
    fn frame_rgba(&mut self) -> Option<(u32, u32, &[u8])> {
        None
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

/// The session a window drives answers `app.diagnostics` with the
/// process block: this process's pid, the strategy of the host it holds
/// — **the session's own type parameter**, whichever the application
/// chose (M55/T55.4) — and the shell's features, which no longer carry the
/// strategy at all.
///
/// Both strategies, in one binary: that is only possible because the
/// strategy is a type and not a feature, and it is what shows the report
/// reads the type — a constant would answer one of the two wrongly.
fn reports_its_process<I: fulgurite::invalidate::Invalidation>() {
    let mut host: Host<Fx, _, I> = Host::new(Fx::default(), 100, 40);
    host.pump();
    let mut s = Local::from_host(host, "process-info", Color(255, 255, 255, 255), Crashing);
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"app.diagnostics","params":{}}"#;
    let reply: Value = serde_json::from_str(&s.agent(req, &mut Blind)).expect("JSON");
    let p = &reply["result"]["process"];
    println!("process block: {p}");
    println!("expected strategy {:?}, pid {}", std::any::type_name::<I>(), std::process::id());
    assert_eq!(p["supported"], true, "{reply}");
    assert_eq!(p["strategy"], std::any::type_name::<I>(), "{reply}");
    assert_eq!(p["pid"], std::process::id(), "{reply}");
    let features: Vec<&str> = p["features"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(features.contains(&"agent"), "a live-agent build reports no `agent`: {features:?}");
    assert!(!features.contains(&"everything"), "the strategy is a type, not a feature: {features:?}");
    assert!(p["exe"].as_str().is_some_and(|e| !e.is_empty()), "no exe: {reply}");
}

#[test]
fn a_session_reports_its_process() {
    reports_its_process::<DefaultInval>();
    reports_its_process::<fulgurite::Everything>();
}
