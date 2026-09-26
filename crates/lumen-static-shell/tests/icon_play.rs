//! **The Play mark is solid** — M57/B4 (`docs/plan-widget-fixes.md`).
//!
//! It was filled with vertical `Line`s a fraction of a pixel wide, and
//! once the painter snapped edges to whole pixels (M57/A15) alternate
//! columns came out empty: a striped triangle
//! (`docs/widget-audit.md`, Icon). Rasterised, at three scales, because
//! the stripes depend on how the widths meet the pixel grid.
//!
//! **The stripes are tonal, not gaps.** The first version of this test
//! thresholded at half grey and passed with the old fill restored: the
//! lines were antialiased and overlapped, so every column was inked, in
//! bands of different shades. So it counts pixels that are ink but not
//! *the* ink.

use lumen_static::display::Color;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, StaticTree, Widget};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;
use lumen_static_widgets::{Icon, Symbol};

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("icon")]
    icon: Icon,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const BG: Color = Color(255, 255, 255, 255);
const INK: Color = Color(0, 0, 0, 255);

/// The ink runs of each pixel row inside the icon's box, and how many
/// inked pixels are not exactly the ink colour.
fn runs(scale: f32, size: f32) -> (Vec<Vec<(u32, u32)>>, usize) {
    let mut app = App::default();
    app.ui.icon = Icon::new(Symbol::Play);
    let (w, h) = ((size * scale) as u32 + 8, (size * scale) as u32 + 8);
    let mut host: Host<App> = Host::new(app, w, h);
    host.set_scale(scale);
    host.set_stylesheet(&[Rule::new(
        Selector::Id("icon"),
        Patch::new().color(INK).width(Length::Fixed(size)).height(Length::Fixed(size)),
    )]);
    host.pump();
    let mut c = Canvas::new(w, h);
    c.clear(BG);
    host.render(&mut c);
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "icon").unwrap();
    assert!(host.states[i].size.w >= size, "setup: the icon is {:?}", host.states[i].size);
    let mut rows = Vec::new();
    let mut off = 0;
    for p in c.pixels.chunks(4) {
        if p[0] < 250 && (p[0], p[1], p[2]) != (INK.0, INK.1, INK.2) {
            off += 1;
        }
    }
    for y in 0..h {
        let mut row = Vec::new();
        let mut start = None;
        for x in 0..=w {
            let dark = x < w && {
                let p = ((y * w + x) * 4) as usize;
                c.pixels[p] < 128
            };
            match (dark, start) {
                (true, None) => start = Some(x),
                (false, Some(s)) => {
                    row.push((s, x));
                    start = None;
                }
                _ => {}
            }
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    (rows, off)
}

#[test]
fn every_row_of_the_play_mark_is_one_run() {
    for scale in [1.0, 1.5, 2.0] {
        let size = 24.0;
        let (rows, off) = runs(scale, size);
        let broken: Vec<_> = rows.iter().filter(|r| r.len() > 1).collect();
        let widest = rows.iter().map(|r| r[0].1 - r[0].0).max().unwrap_or(0);
        // k = 0.30 of the box; the run is 1.4 k at the widest.
        let want = (size * 0.30 * 1.4 * scale) as u32;
        println!(
            "scale {scale}: {} inked rows, {} broken, {off} pixels not the ink, widest {widest} px (the mark is {want} px)",
            rows.len(),
            broken.len()
        );
        assert_eq!(off, 0, "scale {scale}: {off} pixels are partly inked — the mark is banded");
        assert!(!rows.is_empty(), "setup: nothing was drawn at scale {scale}");
        assert!(broken.is_empty(), "scale {scale}: rows with gaps, e.g. {:?}", broken[0]);
        assert!(widest + 1 >= want, "scale {scale}: the widest row is {widest} px, under {want}");
    }
}
