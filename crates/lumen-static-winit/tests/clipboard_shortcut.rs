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

// ---- M80/T80.4: Ctrl+X on a `Selectable` is a copy ---------------------
//
// `docs/plan-text-fit.md` puts this gate in the widgets crate's
// `tests/selectable.rs`; it lives here because it asserts
// `keys::clipboard_decision` for the state the drag left, and only this
// crate can name that function (the widgets crate cannot depend on the
// window). The rest is the plan's: a drag selects a word through a `Host`
// with the pinned face, the decision for that state is `Cut`, and the
// `Backspace` the window posts after a Cut is not handled, changes
// nothing, and leaves the word to copy.

#[path = "../../lumen-static-shell/tests/fonts/pinned.rs"]
mod pinned;

mod selectable_cut {
    use lumen_static::display::Color;
    use lumen_static::event::{Event, Key, Mods, PointerButton};
    use lumen_static::style::{Patch, Rule, Selector};
    use lumen_static::{App as DeriveApp, StaticTree, TextMeasure};
    use lumen_static_shell::text::{FontMetrics, FontStack};
    use lumen_static_shell::Host;
    use lumen_static_widgets::Selectable;
    use lumen_static_winit::keys::{clipboard_decision, Clip, Os, Shortcut};

    use super::pinned;

    const TEXT: &str = "copy this word";

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        #[widget_id("sel")]
        sel: Selectable<String>,
    }

    #[derive(DeriveApp, Default)]
    struct Page {
        #[tree]
        ui: Ui,
    }

    #[test]
    fn ctrl_x_on_a_selectable_copies_and_deletes_nothing() {
        let font = FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
        let mut a = Page::default();
        a.ui.sel.label.set(TEXT.to_owned());
        let mut h: Host<Page> = Host::new(a, 400, 80);
        h.set_stylesheet(&[Rule::new(Selector::Id("sel"), Patch::new().color(Color(0, 0, 0, 255)))]);
        let mut m = FontMetrics::new(font);
        h.pump_with(&mut m);
        let i = lumen_static::style::index_of_wid(<Ui as lumen_static::Widget>::SUB_TABLE, "sel").expect("#sel");
        let st = h.states[i];
        let x_at = |m: &mut FontMetrics, n: usize| st.pos.x + st.pad.0 + m.measure_weighted(&TEXT[..n], 16.0, 400.0, f32::INFINITY).w;
        let y = st.pos.y + st.size.h / 2.0;
        let (from, to) = (x_at(&mut m, "copy ".len()), x_at(&mut m, "copy this".len()));
        h.post(Event::PointerDown { x: from, y, button: PointerButton::Left, clicks: 1 });
        let down = h.pump_with(&mut m);
        h.post(Event::PointerMove { x: to, y });
        let moved = h.pump_with(&mut m);
        h.post(Event::up(to, y));
        h.pump_with(&mut m);
        let word = h.copy().map(str::to_owned);
        // The window's decision, from what the window reads: focus and the
        // session's copy.
        let decision = clipboard_decision(Shortcut {
            mods: Mods::CTRL,
            key: Some(Key::Char('x')),
            focused: h.input.focus.is_some(),
            selection: word.is_some(),
            clipboard: false,
            os: Os::Other,
        });
        println!("setup: drag handled {down}/{moved}, focus {:?} (#sel is {i}), selection {word:?}, decision {decision:?}", h.input.focus);
        assert!(down && moved, "setup: the drag was not handled");
        assert_eq!(word.as_deref(), Some("this"), "setup: the drag did not select the word");
        assert_eq!(decision, Some(Clip::Cut), "setup: the window would not treat Ctrl+X as a Cut here");

        // What `Win::clipboard_shortcut` posts after setting the clipboard.
        h.post(Event::Key { key: Key::Backspace, down: true });
        let handled = h.pump_with(&mut m);
        let text = h.app.ui.sel.as_str().to_owned();
        let still = h.copy().map(str::to_owned);
        println!("Backspace after the Cut: handled {handled} (want false); text {text:?} (want {TEXT:?}); copy {still:?} (want \"this\")");
        assert!(!handled, "the Cut's Backspace was handled by read-only text");
        assert_eq!(text, TEXT, "the Cut changed the text");
        assert_eq!(still.as_deref(), Some("this"), "the Cut's Backspace dropped the selection");
    }
}
