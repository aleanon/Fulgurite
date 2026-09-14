//! M24/T24.0 — the seed table must say what the live tree says.
//!
//! [`lumen_static::style::Seed`] moves two facts — a node's kind name and
//! its own defaults — from the instance to the type, because a *hidden*
//! subtree has no instance to ask. Moving a fact makes two spellings of
//! it, and two spellings of one fact are what rot.
//!
//! So: for every widget, and for every node it owns, `SEEDS[i]` must
//! agree with what `Node::node(i)` answers **while that node is live**.
//! Where they disagree the cascade and the agent surface disagree, which
//! is a stylesheet that works until someone asks the agent what it hit.

use lumen_static::prelude::*;
use lumen_static::style::{Seed, Style};
use lumen_static::widget::{Node, Widget};

/// Every seed of `$t`, against the live instance `$v`.
///
/// **What the kind half of this proves, and where it does not.** A
/// widget that reaches `Node::kind_name` through `leaf_common!` or the
/// derive forwards it to `Widget::KIND`, so for those the two sides are
/// one expression and the assertion cannot fail — changing a `KIND`
/// changes both. It catches the impls that *stopped* forwarding, which
/// is a real and silent regression, and the per-node loop catches a bad
/// splice, which is not tautological anywhere. The claim that a kind is
/// the *right* kind is [`no_two_kinds_collide`]'s.
///
/// **Only nodes that are live**: `Option::None` and an inactive enum
/// variant are exactly the cases that have no instance, and asking them
/// is asking the question this whole mechanism exists to stop asking.
macro_rules! agrees {
    ($t:ty, $v:expr) => {{
        let v: $t = $v;
        let seeds: &[Seed] = <$t as Widget>::SEEDS;
        assert_eq!(
            seeds.len(),
            <$t as Widget>::NODE_COUNT,
            "{}: SEEDS has {} entries for {} nodes",
            stringify!($t),
            seeds.len(),
            <$t as Widget>::NODE_COUNT
        );
        assert_eq!(
            <$t as Widget>::KIND,
            Node::kind_name(&v),
            "{}: Widget::KIND and Node::kind_name disagree",
            stringify!($t)
        );
        assert_eq!(
            (seeds[0].defaults)(),
            Node::defaults(&v),
            "{}: Widget::type_defaults and Node::defaults disagree",
            stringify!($t)
        );
        for i in 0..seeds.len() {
            let live = Node::node(&v, i);
            assert_eq!(
                seeds[i].kind,
                Node::kind_name(live),
                "{} node {i}: seed says {:?}, the live node says {:?}",
                stringify!($t),
                seeds[i].kind,
                Node::kind_name(live)
            );
            assert_eq!(
                (seeds[i].defaults)(),
                Node::defaults(live),
                "{} node {i} ({}): seeded defaults differ from the live node's",
                stringify!($t),
                seeds[i].kind
            );
        }
        seeds.len()
    }};
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Pair {
    a: Label<String>,
    b: Button<(), String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Nest {
    head: Label<String>,
    body: Pair,
}

#[test]
fn every_leaf_agrees_with_its_seed() {
    let mut n = 0;
    n += agrees!(Label<String>, Label::default());
    n += agrees!(Spacer, Spacer);
    n += agrees!(Fixed, Fixed::default());
    n += agrees!(Button<(), String>, Button::default());
    n += agrees!(CheckBox<()>, CheckBox::default());
    n += agrees!(Switch<()>, Switch::default());
    n += agrees!(Radio<()>, Radio::default());
    n += agrees!(Slider, Slider::default());
    n += agrees!(TextInput<String>, TextInput::default());
    n += agrees!(lumen_static::textarea::TextArea<String>, Default::default());
    n += agrees!(lumen_static::image::Image, Default::default());
    assert!(n >= 11, "only {n} leaf nodes checked");
    println!("leaves: {n} nodes checked");
}

#[test]
fn every_container_agrees_with_its_seed() {
    let mut n = 0;
    n += agrees!(Pair, Pair::default());
    n += agrees!(Nest, Nest::default());
    n += agrees!(lumen_static::scroll::ScrollView<Pair>, Default::default());
    n += agrees!(lumen_static::popup::Popup<Pair>, Default::default());
    n += agrees!(lumen_static::glass::Glass<Pair>, Default::default());
    n += agrees!(lumen_static::hatch::Either<Label<String>, Pair>, Default::default());
    n += agrees!(lumen_static::hatch::List<Pair>, Default::default());
    n += agrees!(lumen_static::repeat::Repeat<Pair>, Default::default());
    println!("containers: {n} nodes checked");
    assert!(n >= 20, "only {n} container nodes checked");
}

/// An `Option` adds no node, so it must be **transparent** in the seed
/// table — and, unlike the live tree, transparent whether it is filled
/// or not. That is the whole point: `Some` and `None` seed identically.
#[test]
fn an_option_seeds_as_its_content_either_way() {
    type O = Option<Pair>;
    assert_eq!(<O as Widget>::SEEDS.len(), <Pair as Widget>::NODE_COUNT);
    for (i, (o, p)) in
        <O as Widget>::SEEDS.iter().zip(<Pair as Widget>::SEEDS.iter()).enumerate()
    {
        assert_eq!(o.kind, p.kind, "node {i}");
        assert_eq!((o.defaults)(), (p.defaults)(), "node {i}");
    }
    // And the live tree agrees with it when there is something to ask.
    let _ = agrees!(O, Some(Pair::default()));
    let empty: O = None;
    assert_eq!(
        Node::kind_name(&empty),
        <Pair as Widget>::KIND,
        "an empty Option must still name its content's kind"
    );
    assert_eq!(
        Node::defaults(&empty),
        <Pair as Widget>::type_defaults(),
        "an empty Option must still carry its content's defaults"
    );
}

/// A seed's `defaults` must not read the instance — it cannot, being a
/// `fn()`, and this is the assertion that says so out loud: two
/// instances in different states seed identically.
#[test]
fn a_seed_is_the_same_whatever_the_instance_is_doing() {
    let off: CheckBox<()> = CheckBox::default();
    let mut on: CheckBox<()> = CheckBox::default();
    on.on = true;
    let a: Style = Node::defaults(&off);
    let b: Style = Node::defaults(&on);
    assert_eq!(a, b);
    assert_eq!(a, (<CheckBox<()> as Widget>::SEEDS[0].defaults)());
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Boxes {
    #[widget_id("b")]
    t: CheckBox<()>,
}

#[derive(StaticTree)]
#[allow(dead_code)] // `Second` is never built: being hidden is the point.
enum Pick {
    First(Pair),
    Second(Boxes),
}
impl Default for Pick {
    fn default() -> Self {
        Pick::First(Pair::default())
    }
}

/// **The defect this whole mechanism exists for.** An inactive variant
/// has no instance, so the live tree cannot say what kind its nodes are
/// or what they default to — `node(i)` answers with the *enum*. The seed
/// table is built from the variant types, which are known whichever one
/// is showing.
///
/// Before M24/T24.0 the numbers below were `"Pick"` and `Shrink`; a
/// `CheckBox` whose width defaults to `Shrink` is a control with no size
/// of its own, which on screen is a widget that looks like a label
/// (`docs/plan-select.md` §1).
#[test]
fn an_inactive_variant_still_has_its_own_seed() {
    let showing = Pick::default(); // First(Pair) — Second(Boxes) is hidden
    let seeds = <Pick as Widget>::SEEDS;
    let table = <Pick as Widget>::SUB_TABLE;
    assert_eq!(seeds.len(), table.len());

    let bi = table.iter().position(|d| d.wid == "b").expect("the checkbox node");
    // The live tree cannot answer — that is the premise, so assert it.
    assert_eq!(
        Node::kind_name(Node::node(&showing, bi)),
        <Pick as Widget>::KIND,
        "premise failed: the inactive variant's node is reachable after all, \
         so this test is not exercising what it claims to"
    );
    // The seed can.
    assert_eq!(seeds[bi].kind, <CheckBox<()> as Widget>::KIND);
    let seeded = (seeds[bi].defaults)();
    assert_eq!(
        seeded.width,
        <CheckBox<()> as Widget>::type_defaults().width,
        "an inactive CheckBox seeded {:?}, its own default is {:?}",
        seeded.width,
        <CheckBox<()> as Widget>::type_defaults().width
    );
    assert!(
        !matches!(seeded.width, lumen_static::style::Length::Shrink),
        "a Fixed-width control seeded Shrink: the enum's defaults leaked in"
    );
}

/// Kinds must be distinct, and none may be the trait default.
///
/// A kind name is a stylesheet's only handle on a widget type, and two
/// widgets sharing one is the realistic failure: a new widget is written
/// by copying the nearest one, and `const KIND` is a line in the block
/// that gets copied. Both compile, both paint, and a `Kind` rule meant
/// for one silently styles the other.
#[test]
fn no_two_kinds_collide() {
    let kinds = [
        <Label<String> as Widget>::KIND,
        <Spacer as Widget>::KIND,
        <Fixed as Widget>::KIND,
        <Button<(), String> as Widget>::KIND,
        <CheckBox<()> as Widget>::KIND,
        <Switch<()> as Widget>::KIND,
        <Radio<()> as Widget>::KIND,
        <Slider as Widget>::KIND,
        <TextInput<String> as Widget>::KIND,
        <lumen_static::textarea::TextArea<String> as Widget>::KIND,
        <lumen_static::image::Image as Widget>::KIND,
        <lumen_static::scroll::ScrollView<Pair> as Widget>::KIND,
        <lumen_static::popup::Popup<Pair> as Widget>::KIND,
        <lumen_static::glass::Glass<Pair> as Widget>::KIND,
        <lumen_static::hatch::Either<Pair, Pair> as Widget>::KIND,
        <lumen_static::hatch::List<Pair> as Widget>::KIND,
        <lumen_static::repeat::Repeat<Pair> as Widget>::KIND,
    ];
    for (i, a) in kinds.iter().enumerate() {
        assert!(!a.is_empty() && *a != "widget", "kind {i} is {a:?}: never written");
        for b in &kinds[i + 1..] {
            assert_ne!(a, b, "two widgets both call themselves {a:?}");
        }
    }
    // The three toggles are one type with three `KIND`s — the case a
    // per-type sweep misses if it only ever instantiates one of them.
    assert_eq!(<CheckBox<()> as Widget>::KIND, "checkbox");
    assert_eq!(<Switch<()> as Widget>::KIND, "switch");
    assert_eq!(<Radio<()> as Widget>::KIND, "radio");
    println!("{} distinct kinds", kinds.len());
}
