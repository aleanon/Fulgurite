//! **A tree walks by keyboard and has a selection** — M57/B50
//! (`docs/plan-widget-fixes.md`, `docs/widget-audit.md` Tree). It had no
//! keyboard at all and no selection: the hover was the strong fill.

use fulgurite::display::Cmd;
use fulgurite::event::Key;
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::{Branch, Tree};

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("t")]
    t: Tree,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const ROWS: [Branch; 6] = [
    Branch::new("A", 0),
    Branch::new("a1", 1),
    Branch::new("a2", 1),
    Branch::new("B", 0),
    Branch::new("b1", 1),
    Branch::new("C", 0),
];

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.t = Tree::new(&ROWS);
    let mut h: Host<App> = Host::new(app, 400, 400);
    h.pump();
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "t").unwrap();
    h.input.focus = Some(i as u32);
    h.pump();
    h
}

fn key(h: &mut Host<App>, k: Key) -> Option<&'static str> {
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    h.pump();
    h.app.ui.t.selected.map(|i| h.app.ui.t.label(i))
}

#[test]
fn the_arrows_walk_fold_and_climb() {
    let mut h = host();
    let path: Vec<_> = [Key::Down, Key::Down, Key::Left, Key::Left, Key::Down, Key::Right]
        .into_iter()
        .map(|k| key(&mut h, k))
        .collect();
    println!("Down Down Left Left Down Right -> {path:?}; A open {}", h.app.ui.t.is_open(0));
    // Down: A. Down: a1. Left: up to A. Left: A folds (still A). Down: B,
    // past the folded a1/a2. Right: B is open, so into b1.
    assert_eq!(path, [Some("A"), Some("a1"), Some("A"), Some("A"), Some("B"), Some("b1")]);
    assert!(!h.app.ui.t.is_open(0), "Left on an open row did not fold it");
    key(&mut h, Key::Enter);
    assert_eq!(h.app.ui.t.take_chosen(), Some(4), "Enter did not choose b1");
    assert_eq!(h.app.ui.t.take_chosen(), None, "a choice was taken twice");
}

#[test]
fn selection_is_the_fill_and_hover_a_tint() {
    let mut h = host();
    key(&mut h, Key::Down); // A selected
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "t").unwrap();
    let s = h.states[i];
    // Hover the third visible row (a2).
    h.post(Event::PointerMove { x: s.pos.x + 60.0, y: s.pos.y + 8.0 + 26.0 * 2.5 });
    h.pump();
    let fills: Vec<u8> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::RoundedRect { rect, color, .. } if rect[3] - rect[1] == 26.0 => Some(color.3),
        _ => None,
    }).collect();
    println!("hot {:?}, row fills' alpha {fills:?}", h.app.ui.t.hot());
    assert_eq!(h.app.ui.t.hot(), Some(2), "setup: the pointer is not over a2");
    assert_eq!(fills.len(), 2, "a selected row and a hovered row are not two fills");
    assert!(fills.contains(&255) && fills.iter().any(|a| *a < 0x40), "the selection is not the strong fill and the hover a tint");
}
