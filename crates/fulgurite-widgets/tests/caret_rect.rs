//! **Every node that composes says where its caret is** — M80/T80.5
//! (`docs/plan-text-fit.md`, "every composing node answers").
//!
//! `Host::caret_rect` asks the *focused index*, and `NumberInput` shares
//! its field's index, so a composing node whose `Node::caret_rect` is the
//! provided `None` would put every candidate window at the window's origin
//! while its composition worked. This walks every focusable node of the
//! catalogue (`tests/catalogue`), offers each a `Preedit`, lays the tree
//! out again, and asks each node that took it for its caret: it must
//! answer, with a rect inside the node's own box. A read-only field must
//! not take the `Preedit` at all (M57/B20's rule, which `compose` did not
//! follow until this gate found it).
//!
//! Prints how many composed (bound: at least 5 — `TextInput`, `TextArea`,
//! `NumberInput`, and the search and combobox fields) and how many failed
//! (bound 0), each by its kind and author id. Injection: `NumberInput`
//! does not forward; one failure, named.

mod catalogue;

use fulgurite::event::Composition;
use fulgurite::widget::{Node, Widget};

#[test]
fn every_composing_node_answers_caret_rect_inside_its_box() {
    let mut ui = catalogue::build();
    let _ = catalogue::frame(&mut ui, 800.0);
    let table = <catalogue::Ui as Widget>::SUB_TABLE;
    let mut composed = Vec::new();
    for (i, d) in table.iter().enumerate() {
        if d.is_focusable() && ui.node_mut(i).compose(Composition::Preedit("7")) {
            composed.push(i);
        }
    }
    // Laid out again, so each caret is measured against the composed text.
    let (_, st, _) = catalogue::frame(&mut ui, 800.0);
    let mut failed = Vec::new();
    let mut boxless = Vec::new();
    for &i in &composed {
        let n = ui.node(i);
        let name = format!("{} #{}", n.kind_name(), table[i].wid);
        let s = st[i];
        let b = [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h];
        // A node laid out with no box (a menu's trigger: a menu has none,
        // M57/B23) is never pressed and never shows a caret, so nothing
        // places a candidate window for it. Counted and named, not judged.
        if s.size.w <= 0.0 || s.size.h <= 0.0 {
            boxless.push(name);
            continue;
        }
        match n.caret_rect(&st, i) {
            None => failed.push(format!("{name}: composed and answered None")),
            Some(r) => {
                let inside = r[0] >= b[0] && r[2] <= b[2] && r[1] >= b[1] && r[3] <= b[3] && r[2] > r[0];
                println!("  {name:<34} caret {r:?} in box {b:?}: {}", if inside { "inside" } else { "OUTSIDE" });
                if !inside {
                    failed.push(format!("{name}: caret {r:?} outside its box {b:?}"));
                }
            }
        }
    }
    let judged = composed.len() - boxless.len();
    println!(
        "composed {} of {} focusable nodes, {judged} with a box (bound >= 5; boxless, not judged: {boxless:?}); \
         failed {} (bound 0)",
        composed.len(),
        table.iter().filter(|d| d.is_focusable()).count(),
        failed.len()
    );
    assert!(judged >= 5, "setup: only {judged} nodes with a box took a Preedit");
    assert!(failed.is_empty(), "composing nodes with no caret: {failed:?}");
}
