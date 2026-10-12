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

use fulgurite::ops::{record_painted, resolve_self};
use fulgurite::style::{Length, Patch, Rule, Selector, Style};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Constraints, LayoutCx, NodeState, Point, Size, StaticTree};

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
    const SUB_TABLE: &'static [fulgurite::desc::NodeDesc] =
        &[fulgurite::desc::NodeDesc::new(0, fulgurite::desc::NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "counting";
    const SEEDS: &'static [fulgurite::style::Seed] =
        &[fulgurite::style::Seed::unlinted("counting", <CountingLeaf as Widget>::type_defaults)];

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

    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn fulgurite::Painter) {}

    fulgurite::leaf_common!(CountingLeaf, 0);
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
    fulgurite::style::cascade_deep(tree, sheet, &mut styles);
    // The row itself takes its whole offer, so "the row is 300 px" is the
    // setup and not an accident of its content.
    styles[0].width = Length::FILL;
    let mut st = vec![NodeState::default(); T::NODE_COUNT];
    let mut m = fulgurite::cx::BlockMetrics;
    {
        let mut cx = LayoutCx::new(&mut m);
        Node::layout(tree, &mut cx, OFFER, &styles, &mut st, 0);
    }
    Node::place(tree, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let table = T::SUB_TABLE;
    let kids = ids
        .iter()
        .map(|id| {
            let at = fulgurite::style::index_of_wid(table, id).expect("a node");
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

// ---- M85/T85.1: `Length::Flexible` -----------------------------------

/// A two-child row: `a` at `Flexible(1)` with `a_content` px of content,
/// and `b` styled `b_len` with `b_content`.
fn flexible_pair(a_content: f32, b_len: Length, b_content: f32) -> (f32, Vec<Child>) {
    let mut t = Two::default();
    t.a.content = a_content;
    t.b.content = b_content;
    let sheet = [width("a", Length::Flexible(1)), width("b", b_len)];
    lay(&mut t, &sheet, &["a", "b"], two_count)
}

fn print_pair(case: &str, row: f32, kids: &[Child], bound: &str) {
    println!(
        "{case}: row {row} px; a: {} layout(s), {} px; b: {} layout(s), {} px (bound: 1 each; {bound})",
        kids[0].layouts, kids[0].w, kids[1].layouts, kids[1].w
    );
}

/// `[CountingLeaf(Flexible(1), 80 px), CountingLeaf(FILL)]` in 300 px:
/// the share is 150, the flexible child ends at its 80, and the fill
/// takes the 220 it left.
#[test]
fn a_flexible_child_under_its_share_leaves_the_rest_to_the_fill() {
    let (row, kids) = flexible_pair(80.0, Length::FILL, 10.0);
    print_pair("[Flexible 80, FILL]", row, &kids, "80/220");
    assert_eq!(row, 300.0, "setup: the row is not 300 px wide");
    assert_eq!(kids[0].layouts, 1, "the Flexible child was laid out {} times", kids[0].layouts);
    assert_eq!(kids[1].layouts, 1, "the FILL child was laid out {} times", kids[1].layouts);
    assert_eq!((kids[0].w, kids[1].w), (80.0, 220.0), "[Flexible 80, FILL] did not split 80/220");
}

/// The same row with 400 px of content: the flexible child is capped at
/// its 150 px share, and the fill gets the other 150.
#[test]
fn a_flexible_child_over_its_share_is_capped_at_it() {
    let (row, kids) = flexible_pair(400.0, Length::FILL, 10.0);
    print_pair("[Flexible 400, FILL]", row, &kids, "150/150");
    assert_eq!(row, 300.0, "setup: the row is not 300 px wide");
    assert_eq!((kids[0].layouts, kids[1].layouts), (1, 1), "a child was laid out more than once");
    assert_eq!((kids[0].w, kids[1].w), (150.0, 150.0), "[Flexible 400, FILL] did not split 150/150");
}

/// **The stated limit, pinned**: two `Flexible`s never share each other's
/// leftover, so `[Flexible 80, Flexible 400]` in 300 px reads 80/150 — not
/// CSS's 80/220, which would need a second layout of the second child.
/// If this ever reads 80/220, distribution grew a second round, and K85a
/// says what that must be measured against first.
#[test]
fn two_flexibles_never_share_their_leftover() {
    let (row, kids) = flexible_pair(80.0, Length::Flexible(1), 400.0);
    print_pair("[Flexible 80, Flexible 400]", row, &kids, "80/150");
    assert_eq!(row, 300.0, "setup: the row is not 300 px wide");
    assert_eq!((kids[0].layouts, kids[1].layouts), (1, 1), "a child was laid out more than once");
    assert_eq!((kids[0].w, kids[1].w), (80.0, 150.0), "[Flexible 80, Flexible 400] did not read 80/150");
}

/// `Style` and `NodeState` at the parent commit (a21135b), whatever
/// they were: this task adds a `Length` variant and must move neither.
///
/// **`NodeState` is 88 since M86/T86.1** (`pointer_mix`, the transition
/// level; `docs/results-transitions.md`), which is not this test's claim:
/// the claim is about `Length`, so the pin moves with the struct and the
/// `Length` bound stays.
const AT_PARENT: (usize, usize) = (84, 88);

#[test]
fn flexible_moves_no_struct() {
    let l = core::mem::size_of::<Length>();
    let (style, state) = (core::mem::size_of::<Style>(), core::mem::size_of::<NodeState>());
    let ram = |a: usize, b: usize| (a + b) as f32 * 100.0 / 1024.0;
    println!(
        "Length {l} B (bound 8); Style {style} B, NodeState {state} B (parent {} / {}); \
         {:.1} KB per 100 nodes (parent {:.1} KB, bound 24 KB)",
        AT_PARENT.0,
        AT_PARENT.1,
        ram(style, state),
        ram(AT_PARENT.0, AT_PARENT.1)
    );
    assert_eq!(l, 8, "Length grew past Fixed(f32)");
    assert_eq!((style, state), AT_PARENT, "Flexible moved Style or NodeState");
}
