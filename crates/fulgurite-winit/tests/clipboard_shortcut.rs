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

use fulgurite::{Key, Mods};
use fulgurite_winit::keys::{clipboard_decision, Clip, Os, Shortcut};

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

#[path = "../../fulgurite-shell/tests/fonts/pinned.rs"]
mod pinned;

mod selectable_cut {
    use fulgurite::display::Color;
    use fulgurite::event::{Event, Key, Mods, PointerButton};
    use fulgurite::style::{Patch, Rule, Selector};
    use fulgurite::{App as DeriveApp, StaticTree, TextMeasure};
    use fulgurite_shell::text::{FontMetrics, FontStack};
    use fulgurite_shell::Host;
    use fulgurite_widgets::Selectable;
    use fulgurite_winit::keys::{clipboard_decision, Clip, Os, Shortcut};

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
        let i = fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, "sel").expect("#sel");
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

// ---- M90/T90.5 (X3): a paste nobody took routes Ctrl+V on --------------
//
// `docs/plan-desktop-integration.md`, "`Win`'s clipboard shortcuts come
// first, and today too often". The window decided on "any node focused",
// so Ctrl+V with a `Button` focused and text on the clipboard was a paste
// and was swallowed: an Edit menu's Paste accelerator never fired, and no
// agent input passes through the window to have seen it. Driven through
// `keys::clipboard_shortcut` and `keys::deliver_key` -- the two functions
// the window's handler calls, in its order -- into a real `Local` session
// built by `Program::host`, with a clipboard holding "x".

mod paste_precedence {
    use fulgurite::event::PointerButton;
    use fulgurite::menu::{MenuEntry, MenuMask, MenuSection};
    use fulgurite::prelude::*;
    use fulgurite::{Event, Key, Mods};
    use fulgurite_shell::program::{application, KeyOutcome, Program};
    use fulgurite_shell::session::{Crashing, Local};
    use fulgurite_widgets::prelude::*;
    use fulgurite_winit::keys::{clipboard_shortcut, deliver_key, ClipText, Os};
    use winit::keyboard::Key as WKey;

    const PASTE: u32 = 30;
    const EDIT: [MenuEntry; 1] = [MenuEntry::new("Paste", PASTE).accel(Key::Char('v'), Mods::CTRL)];
    const SECTIONS: [MenuSection; 1] = [MenuSection { title: "Edit", entries: &EDIT }];

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 8.0)]
    struct Ui {
        #[widget_id("btn")]
        btn: Button<Ran>,
        #[widget_id("field")]
        field: TextInput<String>,
    }

    #[derive(Default)]
    struct Ran {
        ids: Vec<u32>,
    }

    #[derive(DeriveApp, Default)]
    struct App {
        #[tree]
        ui: Ui,
        #[state]
        ran: Ran,
    }

    fn on_menu(r: &mut Ran, id: u32) -> KeyOutcome {
        r.ids.push(id);
        KeyOutcome::Handled
    }

    fn boot() -> App {
        let mut a = App::default();
        a.ui.btn.label = "button";
        a
    }

    fn program() -> Program<App> {
        application(boot).size(400, 200).menu(&SECTIONS, |_: &Ran| MenuMask::ALL).on_menu(on_menu)
    }

    /// A clipboard that is not the desktop's.
    struct Fake(Option<String>);
    impl ClipText for Fake {
        fn read(&mut self) -> Option<String> {
            self.0.clone()
        }
        fn write(&mut self, text: &str) {
            self.0 = Some(text.to_owned());
        }
    }

    fn focus(s: &mut Local<App, Crashing>, wid: &str) {
        let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect("a part");
        let st = s.host.states[i];
        let (x, y) = (st.pos.x + 6.0, st.pos.y + st.size.h / 2.0);
        s.host.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
        s.host.post(Event::up(x, y));
        s.host.pump();
        assert_eq!(s.host.input.focus, Some(i as u32), "setup: #{wid} did not take focus");
    }

    /// Ctrl+V as the window handles it: the clipboard shortcut first, and
    /// the key on through `deliver_key` when that did not consume it.
    fn ctrl_v(s: &mut Local<App, Crashing>, clip: &mut Fake) -> bool {
        s.host.post(Event::Modifiers(Mods::CTRL));
        s.host.pump();
        let consumed = clipboard_shortcut(s, Some(Key::Char('v')), Mods::CTRL, Os::Other, clip);
        if !consumed {
            deliver_key(s, &WKey::Character("v".into()), None, true, Mods::CTRL);
            deliver_key(s, &WKey::Character("v".into()), None, false, Mods::CTRL);
        }
        s.host.post(Event::Modifiers(Mods::NONE));
        s.host.pump();
        consumed
    }

    #[test]
    fn ctrl_v_on_a_button_reaches_the_paste_accelerator_and_a_field_keeps_it() {
        let mut s: Local<App, Crashing> = Local::from_program(program(), Crashing);
        s.host.pump();
        let mut clip = Fake(Some("x".into()));

        // ANCHOR: x3_gate
        focus(&mut s, "btn");
        let consumed = ctrl_v(&mut s, &mut clip);
        let ids = s.host.app.ran.ids.clone();
        println!("Button focused, \"x\" on the clipboard: Ctrl+V consumed as a paste {consumed} (want false); on_menu {ids:?} (want [{PASTE}])");
        assert!(!consumed, "Ctrl+V on a Button was swallowed as a paste nothing took");
        assert_eq!(ids, vec![PASTE], "Ctrl+V on a Button did not run the Paste accelerator once");
        // ANCHOR_END: x3_gate

        // Accept: a focused field takes the paste, and the menu hears nothing.
        focus(&mut s, "field");
        let consumed = ctrl_v(&mut s, &mut clip);
        let (text, ids) = (s.host.app.ui.field.as_str().to_owned(), s.host.app.ran.ids.len());
        println!("TextInput focused: Ctrl+V consumed {consumed} (want true); text {text:?} (want \"x\"); on_menu {ids} (want 1, unchanged)");
        assert!(consumed, "a field's paste was not consumed");
        assert_eq!(text, "x", "the field did not take the paste");
        assert_eq!(ids, 1, "a field's paste also ran the menu");
    }
}
