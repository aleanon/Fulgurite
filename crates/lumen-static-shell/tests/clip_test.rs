//! The scoped containment invariant, proven at the pixel level: a
//! scrolling list must not draw where a click would not land.
//!
//! `crates/lumen-static/tests/scroll_containment.rs` pins the geometry
//! and the clip command; this pins what actually reaches the screen,
//! which is the claim that matters and the one only a renderer can
//! settle.

use lumen_static::ops::{Fixed, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::widget::Node;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::CLEAR;

/// Every row paints a solid 24px black bar the full width of the list.
fn ink_row(row: &mut Row, _: usize) {
    row.ink.w = W as f32;
    row.ink.h = 24.0;
}

/// A row that paints solid black across its whole width.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    ink: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    /// Empty space above the list, so an escaping row has somewhere
    /// visibly wrong to land.
    #[widget_id("head")]
    head: Spacer,
    #[widget_id("list")]
    list: Repeat<Row>,
}

const W: u32 = 60;
const H: u32 = 160;

/// Render a list scrolled half an item, and report the inked fraction of
/// the band *above* the list — where a materialized row is placed but a
/// click is rejected.
fn ink_above_the_list() -> (f32, f32) {
    let mut ui = Ui { head: Spacer, list: Repeat::new(1000, 24.0, ink_row) };
    ui.list.scroll = 12.0;

    let mut styles = pass::default_styles(&ui);
    // A fixed 40px gap, then the list takes the rest.
    let sheet = [
        Rule::new(Selector::Id("head"), Patch::new().height(Length::Fixed(40.0))),
        Rule::new(Selector::Id("list"), Patch::new().height(Length::FILL)),
    ];
    lumen_static::style::cascade(
        <Ui as Widget>::SUB_TABLE,
        &|i| ui.node(i).defaults(),
        &|i| ui.node(i).kind_name(),
        &sheet,
        &mut styles,
    );

    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: W as f32, min_h: 0.0, max_h: H as f32 };
    pass::layout(&mut ui, &styles, &mut st, view);

    let list_top = st[2].pos.y;
    assert!(list_top >= 40.0, "the gap is above the list (got {list_top})");
    // The first row is placed 12px above the list top by `place`.
    assert!(ui.list.row_state(0).pos.y < list_top, "row 0 does overrun upward");

    let mut out = lumen_static::DisplayList::new();
    pass::paint(&ui, &st, &mut out);
    let mut c = Canvas::new(W, H);
    c.clear(CLEAR);
    <Canvas as lumen_static::Renderer>::render(&mut c, &out, W, H);

    // Inked fraction of the band above the list, and of the list itself.
    let band = |y0: u32, y1: u32| {
        let mut inked = 0usize;
        let mut total = 0usize;
        for y in y0..y1 {
            for x in 0..W {
                let p = ((y * W + x) * 4) as usize;
                total += 1;
                if c.pixels[p] != CLEAR.0 || c.pixels[p + 1] != CLEAR.1 {
                    inked += 1;
                }
            }
        }
        inked as f32 / total as f32
    };
    (band(0, list_top as u32), band(list_top as u32, H))
}

#[test]
fn a_scrolled_row_does_not_paint_where_a_click_would_miss() {
    let (above, inside) = ink_above_the_list();
    assert_eq!(above, 0.0, "nothing is drawn above the list — the clip holds");
    assert!(inside > 0.5, "and the list itself still renders ({inside:.2} inked)");
}

/// Render one command list to a fresh canvas and return the inked fraction.
fn ink_of(cmds: &[lumen_static::Cmd]) -> f32 {
    let mut list = lumen_static::DisplayList::new();
    for c in cmds {
        match *c {
            lumen_static::Cmd::Rect { rect, color } => list.rect(rect, color),
            lumen_static::Cmd::PushClip { rect } => list.push_clip(rect),
            lumen_static::Cmd::PopClip => list.pop_clip(),
            lumen_static::Cmd::RoundedRect { rect, color, radius } => {
                list.rounded_rect(rect, color, radius)
            }
            lumen_static::Cmd::Text { .. } | lumen_static::Cmd::Shadow { .. } => unreachable!(),
        }
    }
    let mut c = Canvas::new(64, 64);
    c.clear(CLEAR);
    <Canvas as lumen_static::Renderer>::render(&mut c, &list, 64, 64);
    c.ink_fraction(CLEAR)
}

#[test]
fn a_nan_rect_draws_nothing_inside_or_outside_a_clip() {
    use lumen_static::display::Color;
    use lumen_static::Cmd;
    let nan = [f32::NAN, 0.0, 10.0, 10.0];
    assert_eq!(ink_of(&[Cmd::Rect { rect: nan, color: Color::BLACK }]), 0.0, "outside a clip");
    assert_eq!(
        ink_of(&[
            Cmd::PushClip { rect: [0.0, 0.0, 32.0, 32.0] },
            Cmd::Rect { rect: nan, color: Color::BLACK },
            Cmd::PopClip,
        ]),
        0.0,
        "inside a clip — previously filled the whole clip solid"
    );
}

#[test]
fn a_huge_rect_terminates_and_covers_the_surface() {
    use lumen_static::display::Color;
    use lumen_static::Cmd;
    // Previously ~10^16 loop iterations. If this test finishes, it passed.
    let huge = [-1e8, -1e8, 1e8, 1e8];
    assert_eq!(ink_of(&[Cmd::Rect { rect: huge, color: Color::BLACK }]), 1.0);
}

#[test]
fn an_inverted_rect_draws_nothing() {
    use lumen_static::display::Color;
    use lumen_static::Cmd;
    let inverted = [30.0, 30.0, 10.0, 10.0];
    assert_eq!(ink_of(&[Cmd::Rect { rect: inverted, color: Color::BLACK }]), 0.0);
    assert_eq!(
        ink_of(&[
            Cmd::PushClip { rect: [0.0, 0.0, 64.0, 64.0] },
            Cmd::Rect { rect: inverted, color: Color::BLACK },
            Cmd::PopClip,
        ]),
        0.0
    );
}
