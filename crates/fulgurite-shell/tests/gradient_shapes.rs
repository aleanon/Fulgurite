//! **Radial and rounded gradients on the software canvas** — M82/T82.2
//! (`docs/plan-paint-vocabulary.md`).
//!
//! Four claims, each printed beside its bound:
//!
//! - **radial**: the pixel at the resolved centre is stop 0 and the pixel
//!   at the resolved radius is the last stop, within 2/255;
//! - **corner**: with `corner = 12`, the pixel at (1, 1) inside the rect
//!   has coverage ≤ 1/255 and the rect's centre is the stop colour;
//! - **scaled**: at scale 2 the corner scales too, so the physical pixel
//!   (6, 6) is the page;
//! - **retention**: moving a radial's centre inside the same rect damages
//!   exactly the rect, and an unchanged gradient damages nothing.
//!
//! The GPU half of the first two is `fulgurite-wgpu/tests/parity.rs`,
//! run by `just gpu`.

use fulgurite::display::{Cmd, Color, DisplayList, GradKind, GradShape, Painter, Renderer, Scaled, Stops};
use fulgurite::retain::{Incremental, Recording, Retention};
use fulgurite_shell::software::Canvas;

const PAGE: Color = Color(250, 250, 250, 255);
const FROM: Color = Color(250, 30, 30, 255);
const TO: Color = Color(30, 30, 250, 255);
const W: u32 = 160;
const H: u32 = 120;

/// The radial fixture's rect: 128 × 64 at (16, 16), so the fractions
/// below land on pixel centres exactly in binary.
const RECT: [f32; 4] = [16.0, 16.0, 144.0, 80.0];
/// (47.5 − 16) / 128 and (47.5 − 16) / 64: the centre of pixel (47, 47).
const CENTER: [f32; 2] = [0.24609375, 0.4921875];

fn render(list: &DisplayList) -> Canvas {
    let mut c = Canvas::new(W, H);
    c.clear(PAGE);
    c.render(list, W, H);
    c
}

fn px(c: &Canvas, x: u32, y: u32) -> Color {
    let i = ((y * W + x) * 4) as usize;
    Color(c.pixels[i], c.pixels[i + 1], c.pixels[i + 2], 255)
}

fn delta(a: Color, b: Color) -> u8 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1)).max(a.2.abs_diff(b.2))
}

/// How far `got` moved from the page toward `ink`, 0..=1, by the channel
/// that moved most: the coverage a blend of `ink` over the page implies.
fn coverage(got: Color, ink: Color) -> f32 {
    let ch = |g: u8, i: u8, p: u8| if i == p { 0.0 } else { (g as f32 - p as f32) / (i as f32 - p as f32) };
    ch(got.0, ink.0, PAGE.0).max(ch(got.1, ink.1, PAGE.1)).max(ch(got.2, ink.2, PAGE.2))
}

/// The resolved radius in px: `radius` × the distance from the centre to
/// the farthest corner.
fn radius_px(radius: f32) -> f32 {
    let (cx, cy) = (RECT[0] + CENTER[0] * 128.0, RECT[1] + CENTER[1] * 64.0);
    let dx = (cx - RECT[0]).max(RECT[2] - cx);
    let dy = (cy - RECT[1]).max(RECT[3] - cy);
    radius * (dx * dx + dy * dy).sqrt()
}

#[test]
fn a_radial_runs_from_its_centre_to_its_radius() {
    // A radius of 48 px along the centre's row: 47.5 + 48 = 95.5, the
    // centre of pixel (95, 47).
    let radius = 48.0 / radius_px(1.0);
    let mut list = DisplayList::new();
    list.gradient(RECT, Stops::two(FROM, TO), GradShape::Radial { center: CENTER, radius }, 0.0);
    let radials =
        list.cmds.iter().filter(|c| matches!(c, Cmd::Gradient { kind: GradKind::Radial, .. })).count();
    println!("setup: {radials} radial gradient(s) in the list (want 1)");
    assert_eq!(radials, 1, "the fixture must hold exactly one radial gradient");

    let c = render(&list);
    let (centre, edge) = (px(&c, 47, 47), px(&c, 95, 47));
    let (dc, de) = (delta(centre, FROM), delta(edge, TO));
    println!("radial: centre (47, 47) {centre:?} is stop 0 within {dc}/255; radius (95, 47) {edge:?} is the last stop within {de}/255 (bound 2)");
    assert!(dc <= 2, "the centre pixel is not stop 0: {centre:?}, {dc}/255 off");
    assert!(de <= 2, "the pixel at the radius is not the last stop: {edge:?}, {de}/255 off");
}

#[test]
fn a_rounded_corner_is_clear_and_the_middle_is_the_stop() {
    let rect = [10.0, 10.0, 110.0, 70.0];
    let mut list = DisplayList::new();
    list.gradient(rect, Stops::two(FROM, FROM), GradShape::Linear { angle: 0.0 }, 12.0);
    let c = render(&list);
    // (1, 1) inside the rect: its centre is 2.85 px outside the corner's
    // arc. Not (3, 3), whose centre is 0.02 px outside it and so
    // half-covered on correct code.
    let corner = px(&c, 11, 11);
    let cov = coverage(corner, FROM);
    let middle = px(&c, 60, 40);
    println!(
        "corner: (1, 1) inside the rect {corner:?}, coverage {:.4} = {:.1}/255 (bound 1/255); centre {middle:?} vs stop {FROM:?}",
        cov,
        cov * 255.0
    );
    assert_eq!(middle, FROM, "setup: the rect's centre must be the stop colour");
    assert!(cov <= 1.0 / 255.0, "the corner pixel is covered: {cov:.4}");

    // The accept direction: corner 0 keeps the square corner inked.
    let mut square = DisplayList::new();
    square.gradient(rect, Stops::two(FROM, FROM), GradShape::Linear { angle: 0.0 }, 0.0);
    let s = render(&square);
    println!("square: (1, 1) inside the rect {:?}", px(&s, 11, 11));
    assert_eq!(px(&s, 11, 11), FROM, "a square gradient lost its corner");
}

#[test]
fn at_scale_two_the_corner_scales_too() {
    let mut list = DisplayList::new();
    {
        let mut s = Scaled::new(&mut list, 2.0);
        s.gradient([0.0, 0.0, 50.0, 40.0], Stops::two(FROM, FROM), GradShape::Linear { angle: 0.0 }, 12.0);
    }
    let corner = match list.cmds[..] {
        [Cmd::Gradient { rect, corner, .. }] => {
            println!("scaled: the list holds rect {rect:?}, corner {corner} (want 24)");
            corner
        }
        _ => panic!("setup: the scaled painter must emit exactly one gradient, got {:?}", list.cmds),
    };
    let c = render(&list);
    let p = px(&c, 6, 6);
    println!("scaled: physical (6, 6) {p:?} (want the page {PAGE:?}); centre {:?}", px(&c, 50, 40));
    assert_eq!(px(&c, 50, 40), FROM, "setup: the scaled rect's centre must be inked");
    assert_eq!(p, PAGE, "the physical pixel (6, 6) is inked: the corner was forwarded as {corner}, not scaled");
}

#[test]
fn moving_a_radial_centre_damages_exactly_its_rect() {
    let mut ret = Incremental::default();
    let record = |ret: &mut Incremental, center: [f32; 2]| {
        let mut sink = DisplayList::new();
        {
            let mut rec = Recording::new(&mut sink, ret);
            rec.gradient(RECT, Stops::two(FROM, TO), GradShape::Radial { center, radius: 0.5 }, 6.0);
        }
        assert_eq!(sink.cmds.len(), 1, "setup: the recorder must forward the gradient");
        ret.finish()
    };
    let view = [0.0, 0.0, W as f32, H as f32];
    let area = |d: fulgurite::retain::Damage| -> (f32, usize) {
        let rs: Vec<[f32; 4]> = d.regions(view).collect();
        (rs.iter().fold(0.0, |a, r| a + (r[2] - r[0]) * (r[3] - r[1])), rs.len())
    };
    record(&mut ret, [0.25, 0.5]);
    let (same, n_same) = area(record(&mut ret, [0.25, 0.5]));
    let (moved, n_moved) = area(record(&mut ret, [0.75, 0.5]));
    let want = (RECT[2] - RECT[0]) * (RECT[3] - RECT[1]);
    println!("retention: unchanged {same} px² in {n_same} region(s) (want 0); centre moved {moved} px² in {n_moved} region(s) (want the rect's {want})");
    assert_eq!(same, 0.0, "an unchanged gradient damaged {same} px²");
    assert_eq!(moved, want, "moving the radial's centre damaged {moved} px², not its rect's {want}");
}
