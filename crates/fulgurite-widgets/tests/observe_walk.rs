//! **The static observe walk reports exactly what the table walk did** —
//! M35/T35.1 (`docs/plan-observe-cost.md`, rule 1).
//!
//! `observe::emit_root` — the static walk the agent and `snapshot` now
//! use — against `observe::emit_table`, the `&dyn Node` walk that was
//! always there, on one tree holding every shape the review named: an
//! `Option` that is `None` and one that is `Some`, an `Either`, a derived
//! enum with an inactive variant, a `List`, a `Repeat`, a generic
//! hand-written container (`Glass<W>`) and a hand-written leaf
//! (`Toggle`). Compared entry by entry — id, role, bounds, hidden flag,
//! children — with the first difference printed.
#![cfg(feature = "observe")]

use fulgurite::hatch::{Either, List};
use fulgurite::observe::{emit_root, emit_table, AkNode, NodeId};
use fulgurite::prelude::*;
use fulgurite::repeat::Repeat;
use fulgurite::{pass, Constraints};
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::Glass;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct PaneA {
    a: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct PaneB {
    b: Label<String>,
    c: Label<String>,
}

#[derive(StaticTree)]
#[allow(dead_code)]
enum Pane {
    A(PaneA),
    B(PaneB),
}
impl Default for Pane {
    fn default() -> Self {
        Pane::A(PaneA::default())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Ui {
    none: Option<Label<String>>,
    some: Option<Label<String>>,
    either: Either<Label<String>, Label<String>>,
    pane: Pane,
    list: List<Label<String>>,
    rep: Repeat<Label<String>>,
    glass: Glass<Label<String>>,
    toggle: CheckBox<()>,
}

fn bind(l: &mut Label<String>, i: usize) {
    l.set(format!("row {i}"));
}

type Entries = Vec<(NodeId, AkNode)>;

fn walks(ui: &mut Ui) -> (Entries, Entries) {
    let styles = pass::default_styles(ui);
    let mut st = vec![fulgurite::NodeState::default(); <Ui as Widget>::NODE_COUNT];
    pass::layout(ui, &styles, &mut st, Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 800.0 });
    let table = <Ui as Widget>::SUB_TABLE;
    let id = |i: usize| (i + 1) as u64;
    let (mut new, mut old) = (Vec::new(), Vec::new());
    emit_root(table, &*ui, &st, &id, &mut new);
    emit_table(table, &*ui, &st, &id, &mut old);
    (new, old)
}

#[test]
fn the_static_walk_reports_what_the_table_walk_did() {
    let mut ui = Ui {
        some: Some(Label::new("present".into())),
        list: List::new((0..3).map(|i| Label::new(format!("item {i}"))).collect()),
        rep: Repeat::new(20, 18.0, bind),
        ..Ui::default()
    };
    ui.glass.content.set("in glass".into());
    let (new, old) = walks(&mut ui);
    println!("static walk {} entries, table walk {} entries", new.len(), old.len());
    assert!(new.len() > <Ui as Widget>::NODE_COUNT, "setup: the repeater's rows were not emitted");
    assert_eq!(new.len(), old.len(), "the two walks emitted different numbers of entries");
    for (k, (a, b)) in new.iter().zip(&old).enumerate() {
        let (fa, fb) = (format!("{a:?}"), format!("{b:?}"));
        assert_eq!(fa, fb, "entry {k} differs:\n static {fa}\n  table {fb}");
    }
}
