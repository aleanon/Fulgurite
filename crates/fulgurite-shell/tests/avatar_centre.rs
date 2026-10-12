//! **An avatar's initials are centred in its circle** — M57/B3, the
//! second bug the widget audit was asked about (`docs/widget-audit.md`).
//!
//! `Avatar::paint` handed the whole circle to the text primitive on the
//! belief that it aligns within its box. It draws from the box's left
//! edge, so the audit measured the initials 9.5–14 px left of centre, and
//! a "J" poked out of the circle. Measured here as the audit measured
//! it: rendered with the system face, the ink's box against the circle's.
#![cfg(feature = "truetype")]

use fulgurite::display::{Color, DisplayList, Renderer};
use fulgurite::{Constraints, LayoutCx, Node, NodeState, Point};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{FontMetrics, FontStack};
use fulgurite_widgets::Avatar;

#[test]
fn the_initials_sit_on_the_circles_centre() {
    let Some(font) = FontStack::system() else {
        eprintln!("no system font; skipping");
        return;
    };
    let mut fails = Vec::new();
    for name in ["K", "JD", "ÅB"] {
        let mut a = Avatar::new(name);
        let mut style = [<Avatar<&str> as fulgurite::Widget>::type_defaults(); 1];
        style[0].background = Color(0, 0, 0, 255);
        style[0].color = Color(255, 255, 255, 255);
        let mut st = [NodeState::default(); 1];
        let mut m = FontMetrics::new(font.clone());
        let mut cx = LayoutCx::new(&mut m);
        let lim = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };
        a.layout(&mut cx, lim, &style, &mut st, 0);
        a.place(Point { x: 30.0, y: 30.0 }, &style, &mut st, 0);
        let mut list = DisplayList::new();
        a.paint(&st, 0, &mut list);
        let mut c = Canvas::with_font(100, 100, font.clone());
        c.clear(Color(0, 0, 0, 255));
        c.render(&list, 100, 100);
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..100 {
            for x in 0..100 {
                if c.pixels[((y * 100 + x) * 4) as usize] > 128 {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        let s = st[0];
        let (cx0, cy0) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
        let (ix, iy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
        println!("{name}: ink centre ({ix}, {iy}), circle centre ({cx0}, {cy0}): off by ({:.1}, {:.1})", ix - cx0, iy - cy0);
        // Within 2.5 px on both axes. The advance box is what is centred,
        // and ink is not the advance box: a "J"'s hook overhangs it left
        // and down, which puts "JD"'s ink 2 px off. The defect was 9.5–14
        // px, so this bound tells the two apart.
        if (ix - cx0).abs() > 2.5 || (iy - cy0).abs() > 2.5 {
            fails.push(format!("{name}: off by ({:.1}, {:.1})", ix - cx0, iy - cy0));
        }
    }
    assert!(fails.is_empty(), "initials not centred: {fails:?}");
}
