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

use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::Color;
use lumen_static_shell::session::{FrameSource, Inval, Local, Recover, Screen, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
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
    fn present(&mut self, _src: &mut dyn FrameSource, _page: Color) {}
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        None
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

/// The session a window drives answers `app.diagnostics` with the
/// process block: this process's pid, the strategy of the host it holds
/// ([`Inval`], whichever the build selected) and the shell's features.
#[test]
fn a_session_reports_its_process() {
    let mut host: Host<Fx, _, Inval> = Host::new(Fx::default(), 100, 40);
    host.pump();
    let mut s = Local::from_host(host, "process-info", Color(255, 255, 255, 255), Recover::Off);
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"app.diagnostics","params":{}}"#;
    let reply: Value = serde_json::from_str(&s.agent(req, &mut Blind)).expect("JSON");
    let p = &reply["result"]["process"];
    println!("process block: {p}");
    println!("expected strategy {:?}, pid {}", std::any::type_name::<Inval>(), std::process::id());
    assert_eq!(p["supported"], true, "{reply}");
    assert_eq!(p["strategy"], std::any::type_name::<Inval>(), "{reply}");
    assert_eq!(p["pid"], std::process::id(), "{reply}");
    let features: Vec<&str> = p["features"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(features.contains(&"agent"), "a live-agent build reports no `agent`: {features:?}");
    assert_eq!(
        features.contains(&"everything"),
        cfg!(feature = "everything"),
        "`everything` reported as {features:?}, compiled in: {}",
        cfg!(feature = "everything")
    );
    assert!(p["exe"].as_str().is_some_and(|e| !e.is_empty()), "no exe: {reply}");
}
