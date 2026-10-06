//! **`ui.contrast` with a selector measures the node's own text at any
//! scale** — M87/T87.2.
//!
//! The selector resolves to logical bounds and the host paints through
//! `Scaled`, so at scale 2 the display list is in physical pixels. The
//! session filtered physical run centres by the logical rect, and a
//! node's box held almost none of its own text: `scripts/drive`'s
//! after-every-action contrast check printed "no text in the widget's
//! box" on the whole scale-2 leg and passed. Found by the Markdown drive's
//! quote check, which asks for the least contrasting run and got none.

use lumen_static::prelude::*;
use lumen_static::Color;
use lumen_static_shell::session::{FrameSource, Local, Crashing, Screen, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use serde_json::Value;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("top")]
    top: Label<String>,
    #[widget_id("words")]
    words: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Words {
    #[tree]
    ui: Ui,
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

fn ask(s: &mut Local<Words, Crashing>, method: &str, params: &str) -> Value {
    let req = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let r: Value = serde_json::from_str(&s.agent(&req, &mut Blind)).expect("the reply is JSON");
    assert!(r.get("error").is_none(), "{method} failed: {r}");
    r["result"].clone()
}

/// **Gate**: at scale 1 and at scale 2, `ui.contrast` over `#words`
/// measures exactly that label's one run — and names it. Injection
/// (recorded in the commit): the rect left logical — at scale 2 it
/// measures the other label's run, or none.
#[test]
fn a_selector_measures_its_own_text_at_scale_two() {
    for scale in [1.0f32, 2.0] {
        let mut app = Words::default();
        app.ui.top.set("first label up here".to_string());
        app.ui.words.set("second".to_string());
        let mut host: Host<Words> = Host::new(app, (400.0 * scale) as u32, (300.0 * scale) as u32);
        host.set_scale(scale);
        let mut s = Local::from_host(host, "contrast-scale", Color(255, 255, 255, 255), Crashing);
        s.frame(16.0, (400.0 * scale) as u32, (300.0 * scale) as u32);
        s.agent_frame_done(Default::default());
        let b = ask(&mut s, "ui.resolve", r##"{"selector":"#words"}"##)["bounds"].clone();
        let c = ask(&mut s, "ui.contrast", r##"{"selector":"#words"}"##);
        let text = c["min"]["text"].as_str().unwrap_or("");
        println!("scale {scale}: #words at {b} (logical); measured {} (bound 1), min at {text:?} (bound \"second\")", c["measured"]);
        assert_eq!(c["measured"].as_u64(), Some(1), "at scale {scale} the selector measured {} runs", c["measured"]);
        assert_eq!(text, "second", "at scale {scale} the selector measured another node's text");
    }
}
