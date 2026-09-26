//! **A clipboard shortcut routes on when it has nothing to do** — M45/T45.0
//! item 11, for R10-F13 (`docs/plan-text-correctness.md` §1.5).
//!
//! `clipboard_shortcut` returned `true` — which drops the event — for Ctrl
//! or Cmd with `c`/`x` when there was nothing to copy, and for `v`
//! always, so an application never saw those keys, not even with nothing
//! focused. It accepted Ctrl or Meta on every OS, so Super+C copied on
//! Linux.
//!
//! The decision is `keys::clipboard_decision`, the function the window's
//! handler calls. Each row prints the decision beside the one wanted.

use lumen_static::{Key, Mods};
use lumen_static_winit::keys::{clipboard_decision, Clip, Os, Shortcut};

struct Row {
    name: &'static str,
    input: Shortcut,
    want: Option<Clip>,
}

fn s(mods: Mods, c: char, focused: bool, selection: bool, clipboard: bool, os: Os) -> Shortcut {
    Shortcut { mods, key: Some(Key::Char(c)), focused, selection, clipboard, os }
}

fn table() -> Vec<Row> {
    use Os::*;
    let (ctrl, meta) = (Mods::CTRL, Mods::META);
    vec![
        Row { name: "nothing focused, Ctrl+C", input: s(ctrl, 'c', false, false, true, Other), want: None },
        Row { name: "focused, no selection, Ctrl+C", input: s(ctrl, 'c', true, false, true, Other), want: None },
        Row { name: "a selection, Ctrl+C", input: s(ctrl, 'c', true, true, true, Other), want: Some(Clip::Copy) },
        Row { name: "a selection, Ctrl+X", input: s(ctrl, 'x', true, true, true, Other), want: Some(Clip::Cut) },
        Row { name: "Super+C on Linux", input: s(meta, 'c', true, true, true, Other), want: None },
        Row { name: "Cmd+C on macOS", input: s(meta, 'c', true, true, true, MacOs), want: Some(Clip::Copy) },
        Row { name: "Ctrl+C on macOS", input: s(ctrl, 'c', true, true, true, MacOs), want: None },
        Row { name: "Ctrl+V, empty clipboard", input: s(ctrl, 'v', true, false, false, Other), want: None },
        Row { name: "Ctrl+V, nothing focused", input: s(ctrl, 'v', false, false, true, Other), want: None },
        Row { name: "Ctrl+V, text on the clipboard", input: s(ctrl, 'v', true, false, true, Other), want: Some(Clip::Paste) },
        Row { name: "Ctrl+A", input: s(ctrl, 'a', true, true, true, Other), want: None },
        Row { name: "plain c", input: s(Mods::NONE, 'c', true, true, true, Other), want: None },
    ]
}

#[test]
fn the_shortcut_decision_routes_on_what_it_cannot_act_on() {
    let mut wrong = 0;
    for r in table() {
        let got = clipboard_decision(r.input);
        let ok = got == r.want;
        println!("{:<34} got {:<14} want {:<14} {}", r.name, format!("{got:?}"), format!("{:?}", r.want), if ok { "ok" } else { "WRONG" });
        wrong += !ok as usize;
    }
    println!("wrong {wrong} / {} (bound 0)", table().len());
    assert_eq!(wrong, 0, "{wrong} shortcut decisions differ from the table");
}
