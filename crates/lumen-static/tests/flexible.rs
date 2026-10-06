//! **One layout per child, in a derived row** — M85/T85.0
//! (`docs/plan-layout-input.md`, kill rule K85a).
//!
//! The flexible length (T85.1) is decided as a *single-round* length: its
//! leftover goes only to the fills, so no child of a derived row or
//! column is ever laid out twice. That claim is only worth something if
//! a second layout would be *seen*, so this file builds the count first,
//! before that variant exists: `CountingLeaf` counts its own `layout`
//! calls, and every case below prints each child's count and size.
//!
//! The counts and sizes are recorded once, in
//! `docs/results-layout-input.md` (T85.0).

use std::cell::Cell;

use lumen_static::ops::{record_painted, resolve_self};
use lumen_static::style::{Length, Patch, Rule, Selector, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, NodeState, Point, Size, StaticTree};

/// A leaf whose content is `content` px wide and 10 px tall, and which
/// counts how often it is laid out.
#[derive(Default)]
struct CountingLeaf {
    content: f32,
    layouts: Cell<u32>,
}

impl Widget for CountingLeaf {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [lumen_static::desc::NodeDesc] =
        &[lumen_static::desc::NodeDesc::new(0, lumen_static::desc::NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "counting";
    const SEEDS: &'static [lumen_static::style::Seed] =
        &[lumen_static::style::Seed::unlinted("counting", <CountingLeaf as Widget>::type_defaults)];

    fn type_defaults() -> Style {
        Style::default()
    }
}

impl Node for CountingLeaf {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.layouts.set(self.layouts.get() + 1);
        let s = resolve_self(limits, &style[i], Size { w: self.content, h: 10.0 });
        record_painted(st, i, limits, s, &style[i])
    }

    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn lumen_static::Painter) {}

    lumen_static::leaf_common!(CountingLeaf, 0);
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Two {
    #[widget_id("a")]
    a: CountingLeaf,
    #[widget_id("b")]
    b: CountingLeaf,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Three {
    #[widget_id("a")]
    a: CountingLeaf,
    #[widget_id("b")]
    b: CountingLeaf,
    #[widget_id("c")]
    c: CountingLeaf,
}

/// The row's offer: 300 px of main axis.
const OFFER: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 100.0 };

/// What one laid-out child reports: its layout count and its width.
#[derive(Debug, PartialEq)]
struct Child {
    layouts: u32,
    w: f32,
}

/// Cascade `sheet` onto `tree`, lay it out **once** under [`OFFER`] as
/// `tests/fixed_container.rs` does, and answer the row's own width and
/// each listed child's count and width.
fn lay<T: Node + Widget>(
    tree: &mut T,
    sheet: &[Rule],
    ids: &[&str],
    count: impl Fn(&T, &str) -> u32,
) -> (f32, Vec<Child>) {
    let mut styles = vec![Style::default(); T::NODE_COUNT];
    lumen_static::style::cascade_deep(tree, sheet, &mut styles);
    // The row itself takes its whole offer, so "the row is 300 px" is the
    // setup and not an accident of its content.
    styles[0].width = Length::FILL;
    let mut st = vec![NodeState::default(); T::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = LayoutCx::new(&mut m);
        Node::layout(tree, &mut cx, OFFER, &styles, &mut st, 0);
    }
    Node::place(tree, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let table = T::SUB_TABLE;
    let kids = ids
        .iter()
        .map(|id| {
            let at = lumen_static::style::index_of_wid(table, id).expect("a node");
            Child { layouts: count(tree, id), w: st[at].size.w }
        })
        .collect();
    (st[0].size.w, kids)
}

fn width(id: &'static str, l: Length) -> Rule {
    Rule::new(Selector::Id(id), Patch::new().width(l))
}

fn two_count(t: &Two, id: &str) -> u32 {
    match id {
        "a" => t.a.layouts.get(),
        _ => t.b.layouts.get(),
    }
}

/// `[CountingLeaf(Shrink, 80 px), CountingLeaf(FILL)]` in 300 px: each
/// child is laid out exactly once, and the fill takes the 220 px the
/// shrink child left.
#[test]
fn every_child_of_a_derived_row_is_laid_out_once() {
    let mut t = Two::default();
    t.a.content = 80.0;
    t.b.content = 10.0;
    let sheet = [width("a", Length::Shrink), width("b", Length::FILL)];
    let (row, kids) = lay(&mut t, &sheet, &["a", "b"], two_count);
    println!(
        "row {row} px (setup: 300); Shrink(80): {} layout(s), {} px; FILL: {} layout(s), {} px (bound: 1 each; 80/220)",
        kids[0].layouts, kids[0].w, kids[1].layouts, kids[1].w
    );
    assert_eq!(row, 300.0, "setup: the row is not 300 px wide");
    assert_eq!(kids[0].layouts, 1, "the Shrink child was laid out {} times", kids[0].layouts);
    assert_eq!(kids[1].layouts, 1, "the FILL child was laid out {} times", kids[1].layouts);
    assert_eq!((kids[0].w, kids[1].w), (80.0, 220.0), "the row did not split 80/220");
}

/// The accept direction: a row of three `Shrink` children, which never
/// reaches the fill scan, reads one layout each.
#[test]
fn three_shrink_children_are_laid_out_once_each() {
    let mut t = Three::default();
    t.a.content = 40.0;
    t.b.content = 50.0;
    t.c.content = 60.0;
    let count = |t: &Three, id: &str| match id {
        "a" => t.a.layouts.get(),
        "b" => t.b.layouts.get(),
        _ => t.c.layouts.get(),
    };
    let (row, kids) = lay(&mut t, &[], &["a", "b", "c"], count);
    let n: Vec<u32> = kids.iter().map(|k| k.layouts).collect();
    let w: Vec<f32> = kids.iter().map(|k| k.w).collect();
    println!("row {row} px; three Shrink children: layouts {n:?} (bound 1, 1, 1), widths {w:?}");
    assert_eq!(n, vec![1, 1, 1], "a Shrink child was laid out more than once");
    assert_eq!(w, vec![40.0, 50.0, 60.0]);
}
