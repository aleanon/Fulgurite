//! **A skeleton's shimmer is a soft band, and one band for a group** —
//! M57/B29 (`docs/plan-widget-fixes.md`).
//!
//! It was a hard-edged block a fraction of each skeleton's own width, so
//! stacked rows of different widths flashed separate blocks of different
//! sizes at different places (`docs/widget-audit.md`, Skeleton).

use lumen_static::display::Color;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, StaticTree, Widget};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;
use lumen_static_widgets::Skeleton;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("a")]
    a: Skeleton,
    #[widget_id("b")]
    b: Skeleton,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const W: u32 = 500;
const H: u32 = 60;

#[test]
fn the_band_is_soft_and_shared_by_the_group() {
    let mut app = App::default();
    // The band's centre at 0.5 of a 480 px period: x = 240.
    app.ui.a = Skeleton::new(0.5);
    app.ui.b = Skeleton::new(0.5);
    let mut h: Host<App> = Host::new(app, W, H);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("a"), Patch::new().width(Length::Fixed(400.0))),
        Rule::new(Selector::Id("b"), Patch::new().width(Length::Fixed(300.0))),
    ]);
    h.pump();
    let mut c = Canvas::new(W, H);
    c.clear(Color(255, 255, 255, 255));
    h.render(&mut c);
    let table = <Ui as Widget>::SUB_TABLE;
    let mut peaks = Vec::new();
    for id in ["a", "b"] {
        let s = h.states[lumen_static::style::index_of_wid(table, id).unwrap()];
        let y = (s.pos.y + s.size.h / 2.0) as u32;
        let row: Vec<u8> = (s.pos.x as u32 + 2..(s.pos.x + s.size.w) as u32 - 2)
            .map(|x| c.pixels[((y * W + x) * 4) as usize])
            .collect();
        let mut shades: Vec<u8> = row.clone();
        shades.sort();
        shades.dedup();
        let (peak_x, _) = row.iter().enumerate().max_by_key(|(_, v)| **v).unwrap();
        let peak = s.pos.x as usize + 2 + peak_x;
        println!("#{id}: {} px wide, {} distinct shades along it, brightest at x={peak}", s.size.w, shades.len());
        assert!(shades.len() >= 12, "#{id}: {} shades — a hard-edged band, not a gradient", shades.len());
        peaks.push(peak as i32);
    }
    assert!((peaks[0] - peaks[1]).abs() <= 2, "the two rows show the band at {} and {}", peaks[0], peaks[1]);
}
