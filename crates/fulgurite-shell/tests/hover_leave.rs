//! **A widget's hover highlight clears when the pointer leaves** —
//! M57/B50 (`docs/plan-widget-fixes.md`).
//!
//! A widget that highlights a *row* learns which one from `Event::Hover`,
//! which is offered to the node under the pointer. When the pointer moved
//! off it, nothing told it: a tree kept its dark highlight on the last row
//! after the pointer had left (`docs/widget-audit.md`, Tree). `dispatch`
//! now sends `Event::Leave` to the node the pointer left.

use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::{Branch, Tree};

const ROWS: [Branch; 3] = [Branch::new("a", 0), Branch::new("b", 1), Branch::new("c", 0)];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("tree")]
    tree: Tree,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

#[test]
fn a_trees_highlight_clears_when_the_pointer_leaves() {
    let mut app = App::default();
    app.ui.tree = Tree::new(&ROWS);
    let mut h: Host<App> = Host::new(app, 300, 300);
    h.set_stylesheet(&[fulgurite::style::Rule::new(
        fulgurite::style::Selector::Id("tree"),
        fulgurite::style::Patch::new().width(fulgurite::style::Length::Fixed(200.0)),
    )]);
    h.pump();
    let before = (h.list.len(), h.list.hash_from(0));
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "tree").unwrap();
    let s = h.states[i];
    h.post(Event::PointerMove { x: s.pos.x + 20.0, y: s.pos.y + 12.0 });
    h.pump();
    let hovered = (h.list.len(), h.list.hash_from(0));
    assert_ne!(hovered, before, "setup: hovering a row painted nothing different");
    // Off the tree entirely: below it, on nothing.
    h.post(Event::PointerMove { x: s.pos.x + 20.0, y: 295.0 });
    h.pump();
    let after = (h.list.len(), h.list.hash_from(0));
    println!("commands: before {}, hovered {}, after leaving {}", before.0, hovered.0, after.0);
    assert_eq!(after, before, "the highlight stayed after the pointer left the tree");
}
