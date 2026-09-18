//! M27/T27.1 — the border: a `Style` property, an inset stroked rect.
//!
//! Twelve of M27's thirty-three widgets are bordered controls, and
//! before this the framework could not draw one: `Style` had no border
//! and `Cmd` had no stroke. `docs/plan-widgets-gallery.md` §3c found it
//! by putting our `TextInput` beside Lumen's and recorded it as "a gap
//! worth a task of its own".
//!
//! Four claims, each with the injection that reddens it named in the
//! commit body:
//!
//! 1. a stylesheet can **say** border — the cascade carries it;
//! 2. something **draws** it — the Config-gap pattern is four instances
//!    deep in this repository and a property nothing paints is the
//!    fifth;
//! 3. it is drawn **inside** the node, not around it — which is the
//!    whole reason it is a command and not `focus_ring`'s underlay;
//! 4. it **survives an overlay pass**, which is the `Deferring` lesson
//!    (a provided `Painter` method acts on the wrapper, so every `Line`
//!    and `Image` under an overlay was dropped for a year).

use lumen_static::display::{Cmd, Color, DisplayList, Painter};
use lumen_static::prelude::*;
use lumen_static::style::cascade_deep;
use lumen_static::widget::Node;
use lumen_static::{Constraints, NodeState};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };
const INK: Color = Color(0x20, 0x40, 0x80, 0xff);
const FILL: Color = Color(0xf0, 0xf0, 0xf0, 0xff);
const W: u8 = 2;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput,
}

fn sheet(border: bool) -> Vec<Rule> {
    let mut p = Patch::new()
        .width(Length::Fixed(120.0))
        .height(Length::Fixed(28.0))
        .background(FILL)
        .radius(6.0);
    if border {
        p = p.border_width(W).border_color(INK);
    }
    vec![Rule::new(Selector::Id("field"), p)]
}

fn painted(border: bool) -> (DisplayList, Vec<NodeState>) {
    let mut ui = Ui::default();
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut ui, &sheet(border), &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    let mut cx = lumen_static::LayoutCx::new(&mut m);
    Node::layout(&mut ui, &mut cx, VIEW, &styles, &mut st, 0);
    Node::place(&mut ui, lumen_static::Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let mut list = DisplayList::new();
    Node::paint(&ui, &st, 0, &mut list);
    (list, st)
}

fn strokes(list: &DisplayList) -> Vec<(lumen_static::display::Rect, Color, f32, f32)> {
    list.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::StrokedRect { rect, color, radius, width } => {
                Some((*rect, *color, *radius, *width))
            }
            _ => None,
        })
        .collect()
}

/// (1) and (2): the sheet says it and the widget draws it.
#[test]
fn a_stylesheet_rule_puts_a_border_on_the_frame() {
    let (list, _) = painted(true);
    let s = strokes(&list);
    assert_eq!(s.len(), 1, "expected exactly one border, got {}: {s:?}", s.len());
    let (_, color, _, width) = s[0];
    assert_eq!(color, INK, "the border is not the colour the sheet asked for");
    assert_eq!(width, W as f32, "the border is {width} px, not {W}");
}

/// The other direction: no rule, no command. A `Style` field that is
/// always painted is as wrong as one that never is, and it would cost a
/// retention hash and an ink rect on every node of every frame.
#[test]
fn no_rule_means_no_command() {
    let (list, _) = painted(false);
    assert!(strokes(&list).is_empty(), "a border was drawn with no rule asking for one");
}

/// (3) **Inset.** The whole reason this is a command rather than
/// `focus_ring`'s larger-rect-underneath: a bordered and an unbordered
/// control in the same row must line up.
#[test]
fn the_border_is_inside_the_node_not_around_it() {
    let (list, st) = painted(true);
    let (r, ..) = strokes(&list)[0];
    let node = lumen_static::ops::rect_of(&st[1]);
    assert_eq!(r, node, "the border's rect is not the node's: {r:?} vs {node:?}");
    // And the node is the size the sheet asked for — printed beside the
    // bound, because a border that grew the box would show up here as
    // 124x32 and the equality above would still hold.
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    assert!(
        (w - 120.0).abs() < 0.01 && (h - 28.0).abs() < 0.01,
        "the bordered field is {w}x{h}, not the 120x28 the sheet asked for — \
         a border drawn outside would widen it by 2*{W}"
    );
}

/// (4) **It survives an overlay pass.** `Deferring` overrode seven of
/// `Painter`'s eleven methods and inherited the rest, so every `Line`
/// and `Image` under an overlay was dropped and every gradient
/// flattened. `stroked_rect` is a *required* method for exactly this
/// reason, and this asserts the reason rather than trusting it.
#[test]
fn a_border_survives_an_overlay_pass() {
    use lumen_static::overlay::{Deferring, Overlays};
    let mut plain = DisplayList::new();
    let mut ov: Overlays<4> = Overlays::default();
    {
        let mut d = Deferring::new(&mut plain, &mut ov);
        d.stroked_rect([1.0, 2.0, 30.0, 40.0], INK, 4.0, 3.0);
    }
    let s = strokes(&plain);
    assert_eq!(s.len(), 1, "the overlay wrapper swallowed the border");
    assert_eq!(s[0], ([1.0, 2.0, 30.0, 40.0], INK, 4.0, 3.0), "it arrived changed");
}
