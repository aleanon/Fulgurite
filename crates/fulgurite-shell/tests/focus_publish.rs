//! **Focus is published after the event has moved it** — M57/A1
//! (`docs/plan-widget-fixes.md`).
//!
//! The widget audit was asked about a button whose border "lights up only
//! when I click another button, and then the previous one lights up"
//! (`docs/widget-audit.md`, bug 1). `pass::dispatch` wrote
//! `NodeState::focused` at the *top* of the event, before the press moved
//! focus, so the flag was one event behind. The host hides the ring on
//! `input.focus` — the new node — for a pointer press, and the old node
//! kept `focused` with its ring intact.
//!
//! The tests read what was **painted**, not only the flag: a ring is a
//! command around the node's box, and the defect was a ring on the wrong
//! button.

use fulgurite::event::Key;
use fulgurite::{App as DeriveApp, Event, StaticTree};
use fulgurite::display::Cmd;
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Button;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 24.0)]
struct Ui {
    #[widget_id("a")]
    a: Button<App>,
    #[widget_id("b")]
    b: Button<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.a.label = "Alpha";
    app.ui.b.label = "Beta";
    let mut h = Host::new(app, 400, 120);
    h.pump();
    h.pump();
    h
}

fn index(wid: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, wid).unwrap()
}

/// A press and a release **with a frame between them**, as a window
/// renders them. Posting both before one `pump` hides the defect: the
/// release republished the (by then moved) focus, so the frame after it
/// was right — the injection of the old order left that version green.
fn press(h: &mut Host<App>, wid: &str) {
    let s = h.states[index(wid)];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    h.pump();
}

fn release(h: &mut Host<App>, wid: &str) {
    let s = h.states[index(wid)];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    h.pump();
}

fn click(h: &mut Host<App>, wid: &str) {
    press(h, wid);
    release(h, wid);
}

/// Whether the painted list holds a rect that surrounds `wid`'s box —
/// larger on every side, which is what a ring is and a body is not.
fn ringed(h: &Host<App>, wid: &str) -> bool {
    let s = h.states[index(wid)];
    let (x0, y0, x1, y1) = (s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h);
    let mut found = false;
    for c in &h.list.cmds {
        let r = match c {
            Cmd::Rect { rect, .. } | Cmd::RoundedRect { rect, .. } | Cmd::StrokedRect { rect, .. } => *rect,
            _ => continue,
        };
        let around = r[0] < x0 - 0.5 && r[1] < y0 - 0.5 && r[2] > x1 + 0.5 && r[3] > y1 + 0.5;
        // Not the whole window's background.
        let near = x0 - r[0] < 8.0 && r[2] - x1 < 8.0;
        found |= around && near;
    }
    found
}

#[test]
fn a_press_on_b_does_not_light_a() {
    let mut h = host();
    click(&mut h, "a");
    press(&mut h, "b");
    let (a, b) = (index("a"), index("b"));
    // The setup, asserted: the press landed and moved focus.
    assert_eq!(h.input.focus, Some(b as u32), "setup: the press on B did not focus it");
    println!(
        "while B is held, after A: focused a={} b={}; ring painted a={} b={}",
        h.states[a].focused,
        h.states[b].focused,
        ringed(&h, "a"),
        ringed(&h, "b")
    );
    assert!(!h.states[a].focused, "A still published as focused after the press moved focus to B");
    assert!(h.states[b].focused, "B holds focus and is not published as focused");
    assert!(!ringed(&h, "a"), "a ring was painted around A after B was pressed");
    assert!(!ringed(&h, "b"), "a pointer press showed a ring on B");
    release(&mut h, "b");
    assert!(!ringed(&h, "a") && !ringed(&h, "b"), "a ring after the release");
}

/// The other direction: keyboard focus shows its ring **on the frame it
/// moved**, around the node it moved to.
#[test]
fn tab_rings_the_stop_it_moved_to_in_the_same_frame() {
    let mut h = host();
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump();
    let a = index("a");
    assert_eq!(h.input.focus, Some(a as u32), "setup: Tab did not reach A");
    assert!(h.states[a].focused, "Tab moved focus to A and the frame does not publish it");
    assert!(ringed(&h, "a"), "no ring around A on the frame Tab reached it");

    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump();
    assert!(ringed(&h, "b"), "no ring around B on the frame Tab reached it");
    assert!(!ringed(&h, "a"), "A kept its ring after Tab left it");
}
