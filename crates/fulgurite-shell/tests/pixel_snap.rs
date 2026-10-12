//! **A 1 px line is one pixel** — M57/A15 (`docs/plan-widget-fixes.md`).
//!
//! Nothing snapped geometry to the pixel grid, and centring lands on
//! halves: the showcase's divider sat at y 428.5–429.5 and the software
//! renderer, rounding an axis-aligned edge outward, drew it as two full
//! rows (`docs/widget-audit.md` X10). Edges are now snapped where logical
//! becomes physical (`Scaled`), at any scale.

use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Color, StaticTree};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Fixed;
use fulgurite_widgets::Divider;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    /// 10.5 px tall, so the divider under it starts on a half pixel.
    gap: Fixed,
    #[widget_id("rule")]
    rule: Divider,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const PAGE: Color = Color(255, 255, 255, 255);
const INK: Color = Color(0, 0, 0, 255);

/// Rows at column 20 that the divider inked, at `scale`.
fn inked_rows(scale: f32) -> (usize, f32) {
    let mut app = App::default();
    app.ui.gap = Fixed { w: 10.0, h: 10.5 };
    let (w, h) = ((60.0 * scale) as u32, (40.0 * scale) as u32);
    let mut host: Host<App> = Host::new(app, w, h);
    host.set_scale(scale);
    host.set_stylesheet(&[Rule::new(
        Selector::Id("rule"),
        Patch::new().width(Length::Fixed(40.0)).background(INK),
    )]);
    host.pump();
    let i = fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, "rule").unwrap();
    let y = host.states[i].pos.y;
    let mut c = Canvas::new(w, h);
    c.clear(PAGE);
    host.render(&mut c);
    let x = (20.0 * scale) as u32;
    let rows = (0..h).filter(|&y| c.pixels[((y * w + x) * 4) as usize] < 250).count();
    (rows, y)
}

#[test]
fn a_centred_hairline_inks_one_pixel_row_per_pixel_of_scale() {
    for (scale, want) in [(1.0, 1), (2.0, 2)] {
        let (rows, y) = inked_rows(scale);
        println!("scale {scale}: divider at logical y {y}, {rows} row(s) inked (want {want})");
        assert!(y.fract() != 0.0, "setup: the divider is not on a half pixel (y {y})");
        assert_eq!(rows, want, "scale {scale}: a 1 px divider inked {rows} rows");
    }
}
