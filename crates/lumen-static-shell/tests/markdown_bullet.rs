//! **A bullet sits on the x-height, and a wrapped bullet hangs** —
//! M57/B7 (`docs/plan-widget-fixes.md`).
//!
//! The dot was centred on the line box, and the renderer centres a run's
//! *capitals* there, so the dot read 2.5 px above the middle of the
//! lower-case letters beside it; and a bullet's second line started under
//! the dot (`docs/widget-audit.md`, Markdown). Both are facts about ink,
//! so this rasterises with the real faces.
#![cfg(feature = "truetype")]

use lumen_static::display::{Color, DisplayList, Renderer};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, NodeState, Point};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_widgets::Markdown;

const W: u32 = 220;
const H: u32 = 140;

/// Dark pixels as (x, y), laid out and rendered with the system face.
///
/// **Not through a `Host`.** The first version of this test used one, and
/// a headless host has no face: every glyph rendered as a solid box, and
/// the "x-height" it measured was the box's. It passed the old dot and
/// failed the right one, both for a reason that was not the one it named.
fn ink(src: &str) -> Option<Vec<(u32, u32)>> {
    let font = FontStack::system()?;
    let mut md = Markdown::new(src);
    let mut style = [<Markdown as Widget>::type_defaults(); 1];
    style[0].color = Color(0, 0, 0, 255);
    style[0].font_size = 15.0;
    let mut st = [NodeState::default(); 1];
    let mut m = FontMetrics::new(font.clone());
    let mut cx = LayoutCx::new(&mut m);
    let lim = Constraints { min_w: 0.0, max_w: W as f32, min_h: 0.0, max_h: H as f32 };
    md.layout(&mut cx, lim, &style, &mut st, 0);
    md.place(Point { x: 0.0, y: 0.0 }, &style, &mut st, 0);
    let mut list = DisplayList::new();
    md.paint(&st, 0, &mut list);
    let mut c = Canvas::with_font(W, H, font);
    c.clear(Color(255, 255, 255, 255));
    c.render(&list, W, H);
    let mut out = Vec::new();
    for y in 0..H {
        for x in 0..W {
            if c.pixels[((y * W + x) * 4) as usize] < 110 {
                out.push((x, y));
            }
        }
    }
    Some(out)
}

fn centre_y(px: &[(u32, u32)]) -> f32 {
    let (lo, hi) = px.iter().fold((u32::MAX, 0), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
    (lo + hi) as f32 / 2.0
}

#[test]
fn a_bullet_is_centred_on_the_x_height() {
    // "- x": the dot, then an `x` — nothing but x-height ink.
    let Some(px) = ink("- x") else {
        eprintln!("no system font; skipping");
        return;
    };
    let dot: Vec<_> = px.iter().copied().filter(|p| p.0 < 10).collect();
    let x: Vec<_> = px.iter().copied().filter(|p| p.0 >= 12).collect();
    assert!(!dot.is_empty() && !x.is_empty(), "setup: dot {} px, x {} px", dot.len(), x.len());
    // An `x` is a glyph, not a box: a box fills its bounds.
    let (x0, x1) = (x.iter().map(|p| p.0).min().unwrap(), x.iter().map(|p| p.0).max().unwrap());
    let (y0, y1) = (x.iter().map(|p| p.1).min().unwrap(), x.iter().map(|p| p.1).max().unwrap());
    let fill = x.len() as f32 / ((x1 - x0 + 1) * (y1 - y0 + 1)) as f32;
    assert!(fill < 0.8, "setup: the `x` fills {fill:.2} of its bounds — a box, not a glyph");
    let (d, e) = (centre_y(&dot), centre_y(&x));
    println!("dot centre y {d}, x centre y {e}, off by {}", d - e);
    assert!((d - e).abs() <= 1.0, "the bullet is {} px off the x-height's middle", d - e);
}

#[test]
fn a_wrapped_bullet_hangs_under_its_text() {
    let Some(px) = ink("- one two three four five six seven eight nine ten eleven twelve thirteen") else {
        eprintln!("no system font; skipping");
        return;
    };
    // Group ink into lines by row gaps.
    let mut rows: Vec<u32> = px.iter().map(|p| p.1).collect();
    rows.sort();
    rows.dedup();
    let mut lines: Vec<(u32, u32)> = Vec::new();
    for y in rows {
        match lines.last_mut() {
            Some(l) if y <= l.1 + 2 => l.1 = y,
            _ => lines.push((y, y)),
        }
    }
    assert!(lines.len() >= 2, "setup: the bullet did not wrap ({} lines)", lines.len());
    let left = |l: (u32, u32), from: u32| px.iter().filter(|p| p.1 >= l.0 && p.1 <= l.1 && p.0 >= from).map(|p| p.0).min().unwrap();
    // The first line's text starts right of the dot; the second line's
    // first ink is the hang.
    let text = left(lines[0], 10);
    let second = left(lines[1], 0);
    println!("{} lines; first text at x={text}, second line at x={second}", lines.len());
    assert!((second as i32 - text as i32).abs() <= 1, "the second line starts at {second}, not under the text at {text}");
}
