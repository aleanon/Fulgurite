//! **`input.paste` pastes, through the session a window drives** —
//! M45/T45.7. The drive harness pastes into a `TextArea` with it, and a
//! recorded session keeps it as a paste rather than as typing.
//!
//! The answer is what the node **kept**, so a single-line field reports
//! fewer characters than it was given, and a host that cannot paste says
//! so rather than answering 0.

use lumen_static::prelude::*;
use lumen_static::{Color, Event, Key};
use lumen_static_shell::session::{FrameSource, Local, Crashing, Screen, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::TextArea;
use serde_json::Value;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("area")]
    area: TextArea,
}

#[derive(DeriveApp, Default)]
struct Fx {
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

#[test]
fn input_paste_reaches_the_focused_area_as_one_edit() {
    let mut s = Local::from_host(Host::new(Fx::default(), 300, 200), "paste", Color(255, 255, 255, 255), Crashing);
    s.frame(0.0, 300, 200);
    s.post(Event::Key { key: Key::Tab, down: true });
    s.frame(0.0, 300, 200);
    assert_eq!(s.host.input.focus, Some(1), "setup: Tab did not focus the area");
    let f0 = s.host.frame_number();
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"input.paste","params":{"text":"ab\r\ncd\u0007"}}"#;
    let reply: Value = serde_json::from_str(&s.agent(req, &mut Blind)).expect("JSON");
    println!("reply {reply}; area {:?}; frames {}", s.host.app.ui.area.as_str(), s.host.frame_number() - f0);
    assert_eq!(reply["result"]["kept"], 5, "{reply}");
    assert_eq!(reply["result"]["chars"], 7, "{reply}");
    assert_eq!(s.host.app.ui.area.as_str(), "ab\ncd");
    assert_eq!(s.host.frame_number() - f0, 1, "a paste is one pump");
}
