//! **A container of a fixed size lays its children out inside that
//! size** — found by M56 (`docs/plan-showcase-design.md`).
//!
//! The derive computed a container's content box from the limits it was
//! *offered*, and applied its own `Fixed` width or height only afterwards,
//! when it resolved its own box. So a `Fixed(320)` column offered 743 px
//! gave a `FILL` child 727 px and then reported itself 320 wide: its
//! child hung 400 px outside it. Nothing had noticed because every such
//! container in the workspace sat in a parent that happened to offer
//! less than its fixed size — the showcase's stage gave each demo a
//! 248–320 px share, narrower than every `Fixed` demo around it — until
//! M56 let a demo shrink to its content and the stage offered the whole
//! card instead.
//!
//! **Both directions**: a `FILL` container still hands a `FILL` child
//! everything it was offered, and a `Fixed` larger than the offer is
//! clamped to it rather than widening its children past the parent.

use lumen_static::style::{Edges, Length, Patch, Rule, Selector, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point, StaticTree};
use lumen_static_widgets::ops::Spacer;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Boxed {
    #[widget_id("fill")]
    fill: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("box")]
    boxed: Boxed,
}

const OFFER: Constraints = Constraints { min_w: 0.0, max_w: 500.0, min_h: 0.0, max_h: 400.0 };

/// Lay the tree out under `OFFER` with `boxed` styled by `patch`, and
/// answer the box's size and its `FILL` child's.
fn sizes(patch: Patch) -> ((f32, f32), (f32, f32)) {
    let sheet = [
        Rule::new(Selector::Id("box"), patch),
        Rule::new(Selector::Id("fill"), Patch::new().width(Length::FILL).height(Length::FILL)),
    ];
    let mut ui = Ui::default();
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    lumen_static::style::cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut ui, &mut cx, OFFER, &styles, &mut st, 0);
    }
    Node::place(&mut ui, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let table = <Ui as Widget>::SUB_TABLE;
    let at = |w: &str| lumen_static::style::index_of_wid(table, w).expect("a node");
    let (b, f) = (&st[at("box")], &st[at("fill")]);
    ((b.size.w, b.size.h), (f.size.w, f.size.h))
}

#[test]
fn a_fixed_container_lays_its_children_out_inside_itself() {
    let (b, f) = sizes(
        Patch::new()
            .width(Length::Fixed(100.0))
            .height(Length::Fixed(60.0))
            .padding(Edges::all(10.0)),
    );
    println!("offered 500x400; box {b:?}; its FILL child {f:?} (bound: the box minus 2x10 padding)");
    assert_eq!(b, (100.0, 60.0), "setup: the box did not resolve to its own fixed size");
    assert_eq!(
        f.0, 80.0,
        "a Fixed(100) container gave its FILL child {} px — the offer, not its own width",
        f.0
    );
    assert_eq!(
        f.1, 40.0,
        "a Fixed(60) container gave its FILL child {} px of height — the offer, not its own height",
        f.1
    );
}

/// The other direction: a container that is **not** `Fixed` still hands
/// its `FILL` child the whole offer. (A `Shrink` one hands it nothing, by
/// design — a fill in a shrink container has no remainder to share — so
/// `FILL` is the case where the offer is what a child should get.)
#[test]
fn a_fill_container_still_passes_the_whole_offer_on() {
    let (b, f) = sizes(Patch::new().width(Length::FILL).height(Length::FILL));
    println!("offered 500x400; FILL box {b:?}; its FILL child {f:?}");
    assert_eq!(b, (500.0, 400.0), "setup: the FILL box did not take the offer");
    assert_eq!(f, (500.0, 400.0), "a FILL container narrowed its FILL child to {f:?}");
}

#[test]
fn a_fixed_size_larger_than_the_offer_is_clamped_for_its_children_too() {
    let (b, f) = sizes(Patch::new().width(Length::Fixed(900.0)));
    println!("offered 500 px; Fixed(900) box {b:?}; its FILL child {f:?}");
    assert_eq!(b.0, 500.0, "setup: the box was not clamped to the offer");
    assert_eq!(f.0, 500.0, "a clamped Fixed box handed its child {} px, past its parent", f.0);
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Pair {
    #[widget_id("left")]
    left: Spacer,
    #[widget_id("right")]
    right: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Nest {
    #[widget_id("pair")]
    pair: Pair,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Outer {
    #[widget_id("box")]
    boxed: Nest,
}

/// **The cross axis, one level down** — the case the showcase's grid hit.
/// A direct child whose cross length is `FILL` is resized to its parent's
/// box afterwards, which hid the defect from the tests above; a row
/// *inside* it divides the width it was laid out with, so a grandchild is
/// what shows whether the content saw the offer or the box.
#[test]
fn a_fixed_containers_grandchildren_divide_its_width_not_the_offer() {
    let sheet = [
        Rule::new(Selector::Id("box"), Patch::new().width(Length::Fixed(100.0))),
        Rule::new(Selector::Id("pair"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("left"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("right"), Patch::new().width(Length::FILL)),
    ];
    let mut ui = Outer::default();
    let mut styles = vec![Style::default(); Outer::NODE_COUNT];
    lumen_static::style::cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Outer::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut ui, &mut cx, OFFER, &styles, &mut st, 0);
    }
    Node::place(&mut ui, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let table = <Outer as Widget>::SUB_TABLE;
    let at = |w: &str| st[lumen_static::style::index_of_wid(table, w).expect("a node")].size.w;
    let (b, l, r) = (at("box"), at("left"), at("right"));
    println!("offered 500 px; Fixed(100) box {b}; its row's two FILL halves {l} and {r} (bound: 50 each)");
    assert_eq!(b, 100.0, "setup: the box did not resolve to 100 px");
    assert_eq!((l, r), (50.0, 50.0), "the row inside a 100 px box divided {} px between its halves", l + r);
}

