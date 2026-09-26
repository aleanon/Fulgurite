//! **The measured box holds what the painter draws** — M45/T45.0 item 1,
//! for R10-F1 (`docs/plan-text-correctness.md` §1.1).
//!
//! `FontStack` measured a run's width as `max(g.x + max(advance, ink
//! width))`, and fontdue's `g.x` already includes the glyph's left
//! bearing — so a run ending in a negative-bearing glyph (`j`) measured
//! short, and the painter, which wraps inside the measured box, moved the
//! last glyph to a second line below it. A second, smaller mechanism: a
//! logical width times a fractional scale rounds down in `f32`, and the
//! painter wraps inside that too.
//!
//! §0 rule 1: measured with the measurer a window uses (`FontMetrics`)
//! and painted with the painter a window uses (`Canvas::with_font`),
//! **through a `Host` label**, so the box is the one layout gave and the
//! rect is the one paint emitted — at every scale, since the physical
//! rect is where the round-down lives. A test that measured and painted
//! with two stand-ins would prove the stand-ins agree.
//!
//! Two quantities per case, each printed beside its bound:
//! - **wrapped**: the painter's own layout of the run inside the rect it
//!   was handed differs from its layout with no width limit. Asked of
//!   `FontStack::place_run`, the function both backends paint from.
//! - **overhang**: how far the canvas's ink reaches outside the text
//!   rect, in physical pixels. A wrapped run's second line is the large
//!   case; a glyph whose ink passes the box is the small one.
#![cfg(feature = "truetype")]

use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Cmd, Color, StaticTree};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;

#[path = "fonts/pinned.rs"]
mod pinned;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Ui {
    #[widget_id("label")]
    label: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Page {
    #[tree]
    ui: Ui,
}

/// Twelve strings. `Taj`, `Tj`, `fj` end in a negative-bearing `j`;
/// `Settings` bold at 13 px and 1.5 is the fractional-scale case the
/// review measured; the rest are controls that were right before.
pub const STRINGS: [&str; 12] =
    ["Taj", "Tj", "fj", "Settings", "jejunj", "Hij", "POSITIVE", "Wolf", "gypsy", "quay j", "Taj Mahal", "W"];
pub const SIZES: [f32; 10] = [10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0, 32.0];
pub const WEIGHTS: [f32; 2] = [400.0, 700.0];
pub const SCALES: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 2.5];

/// Logical window; physical is this times the scale.
const LW: f32 = 320.0;
const LH: f32 = 80.0;

pub struct Case {
    pub text: &'static str,
    pub px: f32,
    pub weight: f32,
    pub scale: f32,
    pub wrapped: bool,
    /// Physical px of ink past the rect, (x, y); 0 when inside.
    pub over: (f32, f32),
    /// A hash of the canvas, for "pixel-identical" comparisons.
    pub hash: u64,
}

fn stack() -> FontStack {
    FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses")
}

/// One label, laid out and painted at one (text, size, weight, scale).
fn case(font: &FontStack, text: &'static str, px: f32, weight: f32, scale: f32) -> Case {
    let mut app = Page::default();
    app.ui.label.set(text.to_owned());
    let (pw, ph) = ((LW * scale) as u32, (LH * scale) as u32);
    let mut h: Host<Page> = Host::new(app, pw, ph);
    h.set_scale(scale);
    h.set_stylesheet(&[Rule::new(
        Selector::Id("label"),
        Patch::new().font_size(px).font_weight(weight).color(Color(0, 0, 0, 255)),
    )]);
    let mut m = FontMetrics::with_scale(font.clone(), scale);
    h.pump_with(&mut m);

    // The rect paint emitted, in physical pixels (the list is scaled).
    let (rect, size, w) = h
        .list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Text { rect, text: t, size, weight, .. } if h.list.str_of(*t) == text => {
                Some((*rect, *size, *weight))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("setup: no text command for {text:?}"));

    // Wrapped: the painter's layout inside the rect against its layout
    // with no limit.
    let (inside, _) = font.place_run(text, size, w, rect[2] - rect[0]);
    let (free, _) = font.place_run(text, size, w, f32::INFINITY);
    let wrapped = inside.len() != free.len()
        || inside.iter().zip(&free).any(|(a, b)| a.x != b.x || a.y != b.y);

    let mut canvas = Canvas::with_font(pw, ph, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    h.render(&mut canvas);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut hash: u64 = 0xcbf29ce484222325;
    for y in 0..ph {
        for x in 0..pw {
            let i = ((y * pw + x) * 4) as usize;
            let p = &canvas.pixels[i..i + 3];
            for b in p {
                hash = (hash ^ *b as u64).wrapping_mul(0x100000001b3);
            }
            if p.iter().any(|&c| c != 255) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    assert!(x1 > 0, "setup: {text:?} at {px}/{weight}/{scale} painted no ink");
    let ox = (rect[0] - x0 as f32).max(x1 as f32 - rect[2]).max(0.0);
    let oy = (rect[1] - y0 as f32).max(y1 as f32 - rect[3]).max(0.0);
    Case { text, px, weight, scale, wrapped, over: (ox, oy), hash }
}

/// Every case, in a fixed order.
pub fn corpus(font: &FontStack) -> Vec<Case> {
    let mut out = Vec::with_capacity(1200);
    for &t in &STRINGS {
        for &px in &SIZES {
            for &w in &WEIGHTS {
                for &k in &SCALES {
                    out.push(case(font, t, px, w, k));
                }
            }
        }
    }
    out
}

#[test]
#[ignore = "red until T45.1: R10-F1, a run ending in a negative-bearing glyph measures short"]
fn every_measured_run_holds_its_painted_ink() {
    let font = stack();
    let all = corpus(&font);
    let wrapped: Vec<&Case> = all.iter().filter(|c| c.wrapped).collect();
    let worst_x = all.iter().map(|c| c.over.0).fold(0.0, f32::max);
    let worst_y = all.iter().map(|c| c.over.1).fold(0.0, f32::max);
    let outside: Vec<&Case> = all.iter().filter(|c| c.over.0 > 1.0 || c.over.1 > 1.0).collect();
    for c in wrapped.iter().chain(outside.iter()).take(12) {
        println!(
            "  {:?} {} px w{} x{}: wrapped={} overhang x={} y={}",
            c.text, c.px, c.weight, c.scale, c.wrapped, c.over.0, c.over.1
        );
    }
    let named = all.iter().find(|c| c.text == "Settings" && c.px == 13.0 && c.weight == 700.0 && c.scale == 1.5);
    if let Some(c) = named {
        println!("Settings bold 13 px at 1.5: wrapped={} overhang {:?}", c.wrapped, c.over);
    }
    println!(
        "wrapped {} / {}, worst overhang x/y {worst_x}/{worst_y} px (bound 1.0), {} outside",
        wrapped.len(),
        all.len(),
        outside.len()
    );
    assert_eq!(all.len(), 1200, "setup: the corpus is 12 x 10 x 2 x 5");
    assert!(wrapped.is_empty(), "{} of {} runs re-wrapped inside their measured box", wrapped.len(), all.len());
    assert!(outside.is_empty(), "{} runs painted ink more than 1 px outside their box", outside.len());
}

/// **The accept direction**: runs that were right before stay on one line
/// and inside the box.
#[test]
fn a_run_with_no_overhanging_glyph_stays_on_one_line() {
    let font = stack();
    for t in ["Taj Mahal", "W"] {
        for &k in &SCALES {
            let c = case(&font, t, 16.0, 400.0, k);
            println!("{t:?} x{k}: wrapped={} overhang {:?} (bound 1.0)", c.wrapped, c.over);
            assert!(!c.wrapped && c.over.0 <= 1.0 && c.over.1 <= 1.0, "{t:?} at {k}");
        }
    }
}
