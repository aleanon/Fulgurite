//! **`input.compose` routes through the window's own path** — M48/T48.13.
//!
//! The index assigned it (no plan added it): without it IME is covered
//! only by headless host tests, and the shadow oracle can never see a live
//! compose frame — R1-F2's, which M41/T41.1 closed. This is the headless
//! gate: a session driven as the window drives it, a field focused by a
//! click, "abc" typed by preedit and then committed, each through
//! `Session::agent`. **The live leg — `agent_drive.sh`'s Boundaries leg
//! with the oracle judging those frames — waits for M41/T41.2b.**

use lumen_static::prelude::*;
use lumen_static::Color;
use lumen_static_shell::session::{FrameSource, FrameTimes, Local, Recover, Screen, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use serde_json::Value;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput<String>,
}

#[derive(DeriveApp, Default)]
struct Form {
    #[tree]
    ui: Ui,
}

struct Blind;

impl Screen for Blind {
    fn present(&mut self, _src: &mut dyn FrameSource, _page: Color) -> FrameTimes {
        Default::default()
    }
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        None
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

fn ask(s: &mut Local<Form>, method: &str, params: &str) -> Value {
    let req = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    serde_json::from_str(&s.agent(&req, &mut Blind)).expect("the reply is JSON")
}

#[test]
fn preedit_then_commit_types_into_the_focused_field() {
    let host: Host<Form, _, _> = Host::new(Form::default(), 320, 120);
    let mut s = Local::from_host(host, "agent-compose", Color(255, 255, 255, 255), Recover::Off);
    s.frame(0.0, 320, 120);
    s.agent_frame_done(Default::default());
    let r = ask(&mut s, "input.click", r##"{"selector":"#field"}"##);
    assert_eq!(r["result"]["handled"], true, "setup: the click did not focus the field: {r}");
    s.frame(16.0, 320, 120);
    s.agent_frame_done(Default::default());

    let mut judged = 0;
    for (params, want) in [
        (r#"{"text":"a"}"#, "a"),
        (r#"{"text":"ab"}"#, "ab"),
        (r#"{"text":"abc"}"#, "abc"),
        (r#"{"text":"abc","commit":true}"#, "abc"),
    ] {
        let r = ask(&mut s, "input.compose", params);
        assert!(r.get("error").is_none(), "{params}: {r}");
        s.frame(16.0, 320, 120);
        s.agent_frame_done(Default::default());
        let v = ask(&mut s, "ui.resolve", r##"{"selector":"#field"}"##);
        let shown = v["result"]["value"].as_str().unwrap_or("").to_string();
        println!("compose {params}: changed {}, field shows {shown:?} (want {want:?})", r["result"]["changed"]);
        assert_eq!(r["result"]["changed"], true, "{params}: the step changed nothing");
        assert_eq!(shown, want, "{params}");
        judged += 1;
    }
    println!("compose frames driven {judged} / bound >= 2 (the oracle's judgement is the live leg's, M41/T41.2b)");
    // A commit is not a preedit: a cancel after it leaves the text, where
    // a cancel after a preedit would drop it.
    let r = ask(&mut s, "input.compose", r#"{"cancel":true}"#);
    assert!(r.get("error").is_none(), "cancel: {r}");
    s.frame(16.0, 320, 120);
    let v = ask(&mut s, "ui.resolve", r##"{"selector":"#field"}"##);
    println!("cancel after the commit: field shows {}", v["result"]["value"]);
    assert_eq!(v["result"]["value"], "abc", "the commit did not commit");
    let r = ask(&mut s, "input.compose", "{}");
    assert_eq!(r["error"]["code"], -32602, "no text and no cancel must be refused: {r}");
}
