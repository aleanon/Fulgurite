//! **`Style.background_fill` reaches the screen** — M82/T82.3
//! (`docs/plan-paint-vocabulary.md`, "A gradient in `Style` is an index").
//!
//! A node's fill is a byte in `Style`, stamped into `NodeState::fill` by
//! `record_painted`, and resolved at paint by `ops::fill` against the
//! table the painter answers (`Painter::fills`), which the host installs
//! from `Program::fills` resolved against its palette. Each link can be
//! dropped without a compile error, so each gate below breaks one:
//!
//! - **reaches paint**, at three of `ops::fill`'s call sites — a derive
//!   container, a `Repeat` and a `Button` — one `Cmd::Gradient` over the
//!   node's own box, with the table's stops;
//! - **palette**: a role-named stop follows `Host::set_palette`, and the
//!   damage is exactly the node's box;
//! - **`set_fills` marks**: on the default `Boundaries<256>` host, a
//!   changed table reaches the next frame; the same table marks nothing;
//! - **index 0 is today**: a non-empty table and no node asking paints no
//!   gradient at all;
//! - **disabled**: the stops fade to 40% alpha, as `bg` does.
//!
//! Every gate asserts its setup landed — the node's `fill` byte and the
//! rule's field bit — and prints the quantity beside its bound.

use lumen_static::display::{GradShape, Stops};
use lumen_static::repeat::Repeat;
use lumen_static::retain::Damage;
use lumen_static::style::{Fields, Fill, Length, Palette, Patch, Role, Rule, Selector};
use lumen_static::{App as DeriveApp, Cmd, Color, NodeState, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Button, Label};

const W: u32 = 300;
const H: u32 = 200;

/// A derive container of its own, so its paint is the derive's.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    note: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("panel")]
    panel: Panel,
    #[widget_id("list")]
    list: Repeat<Label<String>>,
    #[widget_id("go")]
    go: Button<FillApp>,
}

#[derive(DeriveApp, Default)]
struct FillApp {
    #[tree]
    ui: Ui,
}

const A: Color = Color(200, 30, 30, 255);
const B: Color = Color(30, 30, 200, 255);

/// Entry 0: literal stops, radial.
const LITERAL: Fill = Fill::new(
    Stops { at: [0, 255, 0, 0], color: [A, B, A, A], len: 2 },
    GradShape::Radial { center: [0.5, 0.5], radius: 1.0 },
);
/// Entry 1: its first stop named by role.
const ROLED: Fill = LITERAL.role(0, Role::Accent);

const TABLE: [Fill; 2] = [LITERAL, ROLED];

const BOX: Patch = Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(30.0));

fn idx(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect("the id is in the table")
}

fn rect_of(st: &NodeState) -> [f32; 4] {
    lumen_static::ops::rect_of(st)
}

fn gradients(h: &Host<FillApp>) -> Vec<([f32; 4], Stops)> {
    h.list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Gradient { rect, stops, .. } => Some((*rect, *stops)),
            _ => None,
        })
        .collect()
}

fn damaged(h: &Host<FillApp>) -> f32 {
    match h.damage() {
        Damage::None => 0.0,
        Damage::All => f32::INFINITY,
        d => d.regions([0.0, 0.0, W as f32, H as f32]).map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum(),
    }
}

fn area(r: [f32; 4]) -> f32 {
    (r[2] - r[0]) * (r[3] - r[1])
}

/// A host with `TABLE`, `sheet` applied, and two pumps behind it.
fn host(sheet: &[Rule]) -> Host<FillApp> {
    let mut h: Host<FillApp> = Host::new(FillApp::default(), W, H);
    h.set_fills(&TABLE);
    h.set_stylesheet(sheet);
    h.pump();
    h.pump();
    h
}

/// The gradients painted inside node `i`'s box.
fn inside(h: &Host<FillApp>, i: usize) -> Vec<([f32; 4], Stops)> {
    let r = rect_of(&h.states[i]);
    gradients(h).into_iter().filter(|(g, _)| g[0] >= r[0] && g[1] >= r[1] && g[2] <= r[2] && g[3] <= r[3]).collect()
}

#[test]
fn a_fill_reaches_paint_at_a_container_a_repeat_and_a_button() {
    let mut failures = Vec::new();
    for (kind, wid) in [("derive container", "panel"), ("Repeat", "list"), ("Button", "go")] {
        let rule = Rule::new(Selector::Id(wid), BOX.background_fill(1));
        let h = host(&[rule]);
        let i = idx(wid);
        let st = h.states[i];
        println!(
            "{kind:>16} #{wid}: st.fill {} (setup 1), rule fields carry BACKGROUND_FILL: {}",
            st.fill,
            rule.patch.fields().contains(Fields::BACKGROUND_FILL)
        );
        assert!(rule.patch.fields().contains(Fields::BACKGROUND_FILL), "setup: the rule records its bit");
        let g = inside(&h, i);
        println!("{kind:>16} #{wid}: {} gradient(s) in its box {:?} (bound 1)", g.len(), rect_of(&st));
        if g.len() != 1 {
            failures.push(format!("{kind} (#{wid}): {} gradients, not 1 (st.fill = {})", g.len(), st.fill));
            continue;
        }
        let (rect, stops) = g[0];
        if rect != rect_of(&st) {
            failures.push(format!("{kind}: the gradient's rect {rect:?} is not the node's {:?}", rect_of(&st)));
        }
        if stops != LITERAL.stops {
            failures.push(format!("{kind}: stops {stops:?}, not the table's {:?}", LITERAL.stops));
        }
    }
    assert!(failures.is_empty(), "the fill did not reach paint:\n{}", failures.join("\n"));
}

#[test]
fn a_role_named_stop_follows_set_palette_and_damages_exactly_the_node() {
    let rule = Rule::new(Selector::Id("panel"), BOX.background_fill(2));
    // The button's own colours are palette roles; literal ones here, so
    // the only thing a palette swap can move is the fill.
    let quiet = Rule::new(Selector::Id("go"), Patch::new().background(A).color(B).border_color(B));
    let mut h = host(&[rule, quiet]);
    let i = idx("panel");
    assert_eq!(h.states[i].fill, 2, "setup: the panel's fill byte");
    let first = inside(&h, i);
    assert_eq!(first.len(), 1, "setup: the panel paints its fill");
    println!("light: stop 0 = {:?} (want LIGHT.accent {:?})", first[0].1.color[0], Palette::LIGHT.accent);
    assert_eq!(first[0].1.color[0], Palette::LIGHT.accent, "the role resolved against the host's palette");
    assert_ne!(Palette::LIGHT.accent, Palette::DARK.accent, "setup: the palettes differ where probed");

    h.set_palette(Palette::DARK);
    h.pump();
    let after = inside(&h, i);
    let dmg = damaged(&h);
    let r = rect_of(&h.states[i]);
    println!(
        "dark: stop 0 = {:?} (want DARK.accent {:?}); damage {dmg} px2 against the node's {} px2",
        after.first().map(|g| g.1.color[0]),
        Palette::DARK.accent,
        area(r)
    );
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].1.color[0], Palette::DARK.accent, "set_palette did not re-resolve the fill table");
    assert_eq!(dmg, area(r), "the palette swap damaged other than exactly the filled node");
}

#[test]
fn set_fills_marks_a_bounded_host_and_the_same_table_marks_nothing() {
    let rule = Rule::new(Selector::Id("panel"), BOX.background_fill(1));
    let mut h = host(&[rule]);
    let i = idx("panel");
    assert!(h.changed().can_skip(), "setup: the frame settled, so a frame can skip");
    let changed = [Fill::new(Stops { at: [0, 255, 0, 0], color: [B, A, B, B], len: 2 }, LITERAL.shape), ROLED];
    h.set_fills(&changed);
    h.pump();
    let g = inside(&h, i);
    println!("after a changed table: {} gradient(s), stops {:?} (want {:?})", g.len(), g.first().map(|g| g.1.color), changed[0].stops.color);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].1, changed[0].stops, "the next frame kept the old stops: set_fills did not mark");

    h.pump();
    h.set_fills(&changed);
    h.pump();
    let dmg = damaged(&h);
    println!("the same table again: damage {dmg} px2 (bound 0), can skip: {}", h.changed().can_skip());
    assert_eq!(dmg, 0.0, "an unchanged table repainted something");
}

#[test]
fn index_zero_is_today_no_node_asking_paints_no_gradient() {
    let h = host(&[Rule::new(Selector::Id("panel"), BOX), Rule::new(Selector::Id("go"), BOX)]);
    let n = h.states.len();
    let asking = h.states.iter().filter(|s| s.fill != 0).count();
    let g = gradients(&h).len();
    println!("table {} entries, {n} nodes, {asking} asking; {g} gradient(s) (bound 0)", h.fills().len());
    assert_eq!(h.fills().len(), 2, "setup: the table is installed");
    assert_eq!(asking, 0, "setup: no node asks for a fill");
    assert_eq!(g, 0, "a node with fill == 0 painted a gradient");
}

#[test]
fn a_disabled_nodes_stops_fade_to_forty_percent() {
    let rule = Rule::new(Selector::Id("go"), BOX.background_fill(1).disabled(true));
    let h = host(&[rule]);
    let i = idx("go");
    assert!(h.states[i].disabled && h.states[i].fill == 1, "setup: the button is disabled and filled");
    let g = inside(&h, i);
    assert_eq!(g.len(), 1, "setup: the disabled button paints its fill");
    let alphas: Vec<u8> = g[0].1.color[..2].iter().map(|c| c.3).collect();
    println!("disabled stops' alpha {alphas:?} (want [102, 102], 40% of 255)");
    assert_eq!(alphas, vec![102, 102], "a disabled node's fill did not fade");
}
