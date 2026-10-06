//! **The software canvas draws a shader bracket's fallback, clipped** —
//! M83/T83.3 (`docs/plan-gpu-extension.md`, "A shader is a bracket in the
//! list"). Behind this crate's `shader` (`required-features`), which turns
//! the core's on.
//!
//! `Canvas` runs no WGSL. What it draws for `Cmd::Shader` … `ShaderEnd` is
//! the fallback recorded inside it, clipped to the shader's rect, so the
//! bracket must render exactly as `push_clip(rect)` + the fallback +
//! `pop_clip()` does — and a fallback larger than the rect must not show
//! outside it.

use lumen_static::display::{Color, DisplayList, Rect, ShaderId};
use lumen_static::Renderer;
use lumen_static_shell::software::Canvas;

const W: u32 = 120;
const H: u32 = 100;
const PAGE: Color = Color(250, 250, 250, 255);
const FILL: Color = Color(30, 90, 200, 255);
const RECT: Rect = [30.0, 30.0, 80.0, 70.0];
/// The fallback's fill: 20 px larger than the shader's rect, 10 a side.
const BIG: Rect = [20.0, 20.0, 90.0, 80.0];

fn render(list: &DisplayList) -> Vec<u8> {
    let mut c = Canvas::new(W, H);
    c.clear(PAGE);
    c.render(list, W, H);
    c.pixels
}

/// Pixels of `px` that are not the page, inside and outside `RECT`.
fn inked(px: &[u8]) -> (usize, usize) {
    let (mut inside, mut outside) = (0, 0);
    for (i, p) in px.chunks_exact(4).enumerate() {
        if (p[0], p[1], p[2]) == (PAGE.0, PAGE.1, PAGE.2) {
            continue;
        }
        let (x, y) = ((i as u32 % W) as f32 + 0.5, (i as u32 / W) as f32 + 0.5);
        if x > RECT[0] && x < RECT[2] && y > RECT[1] && y < RECT[3] {
            inside += 1;
        } else {
            outside += 1;
        }
    }
    (inside, outside)
}

#[test]
fn a_bracket_draws_its_fallback_clipped_to_the_rect() {
    let mut bracket = DisplayList::new();
    bracket.shader(RECT, ShaderId(0), &[1, 2, 3, 4]);
    bracket.rect(BIG, FILL);
    bracket.shader_end();
    // And a command after the bracket, unclipped, so an end that failed to
    // pop would cut it too.
    bracket.rect([0.0, 0.0, 10.0, 10.0], FILL);

    let mut clip = DisplayList::new();
    clip.push_clip(RECT);
    clip.rect(BIG, FILL);
    clip.pop_clip();
    clip.rect([0.0, 0.0, 10.0, 10.0], FILL);

    let (got, want) = (render(&bracket), render(&clip));
    let (inside, outside) = inked(&got);
    let differ = got.chunks_exact(4).zip(want.chunks_exact(4)).filter(|(a, b)| a != b).count();
    println!(
        "bracket: fallback pixels inside the rect {inside} (setup > 0), outside it {outside} \
         (the 10x10 corner rect's 100 only); {differ} px differ from push_clip + fill + pop_clip (bound 0)"
    );
    assert!(inside > 0, "setup: the fallback drew nothing inside the shader's rect");
    assert_eq!(outside, 100, "the fallback escaped the shader's rect, or the bracket's end did not pop");
    assert_eq!(differ, 0, "a bracket must render byte-identical to a clip around its fallback");
}
