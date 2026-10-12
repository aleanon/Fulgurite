//! **A card has elevation levels, and level 0 is outlined** — M57/B33
//! (`docs/plan-widget-fixes.md`). A card had one look, and on a dark page
//! a shadow barely shows (`docs/widget-audit.md`, Card).

use fulgurite::display::{Cmd, DisplayList};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Constraints, NodeState, Point};
use fulgurite_widgets::ops::Label;
use fulgurite_widgets::Card;

fn painted(elevation: Option<u8>) -> (Option<f32>, usize) {
    let mut c = Card::new(Label::<&'static str>::default());
    c.elevation = elevation;
    // Through the palette, as a host resolves them: the outline is the
    // border *role*, and bare defaults leave it transparent.
    let styles: Vec<_> = <Card<Label> as Widget>::SEEDS.iter().map(|s| s.style(&fulgurite::style::Palette::LIGHT)).collect();
    let mut st = vec![NodeState::default(); <Card<Label> as Widget>::NODE_COUNT];
    let mut m = fulgurite::cx::BlockMetrics;
    let mut cx = fulgurite::LayoutCx::new(&mut m);
    c.layout(&mut cx, Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 200.0 }, &styles, &mut st, 0);
    c.place(Point { x: 20.0, y: 20.0 }, &styles, &mut st, 0);
    let mut list = DisplayList::new();
    c.paint(&st, 0, &mut list);
    let shadow = list.cmds.iter().find_map(|c| match c {
        Cmd::Shadow { spread, .. } => Some(*spread),
        _ => None,
    });
    let outlines = list.cmds.iter().filter(|c| matches!(c, Cmd::StrokedRect { .. })).count();
    (shadow, outlines)
}

#[test]
fn each_level_draws_its_own_elevation() {
    let got: Vec<_> = [None, Some(0), Some(1), Some(2), Some(3)].into_iter().map(painted).collect();
    println!("None, 0..=3 -> (shadow, outlines): {got:?}");
    assert_eq!(got[0], (Some(8.0), 0), "unset is not the style's shadow");
    assert_eq!(got[1], (None, 1), "level 0 is not flat and outlined");
    assert_eq!((got[2].0, got[3].0, got[4].0), (Some(4.0), Some(8.0), Some(16.0)), "levels 1..=3");
    assert!(got[2..].iter().all(|g| g.1 == 0), "a raised card is outlined too");
}
