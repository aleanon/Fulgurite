//! M26/T26.13: the raster clip stack is fixed-depth, and what happens
//! at the limit is a decision rather than undefined behaviour.
//!
//! It was a `Vec` allocated once per rasterised frame — one heap call
//! in the path whose stated invariant is zero steady-state allocation,
//! and which no test could see until `alloc_gate.rs` was extended to
//! cover a raster frame.
//!
//! **A ninth `PushClip` is ignored**, so the content it would have
//! bounded is clipped by the eight above it instead. That is the
//! conservative direction — too much clipping draws *less*, never more,
//! and drawing outside a clip is the bug a renderer must not have.

use fulgurite::display::{Color, DisplayList};
use fulgurite_shell::software::Canvas;
use fulgurite::Renderer;

const W: u32 = 64;
const H: u32 = 64;

fn ink(c: &Canvas) -> usize {
    c.pixels.chunks_exact(4).filter(|p| p[0] != 255 || p[1] != 255 || p[2] != 255).count()
}

/// Nine nested scopes, each smaller than the last, then a fill. The
/// pixels must be inside the *innermost* scope the stack kept — never
/// outside the eighth.
#[test]
fn a_ninth_clip_scope_never_widens_what_is_drawn() {
    let mut deep = DisplayList::new();
    for k in 0..10u32 {
        let v = k as f32;
        deep.push_clip([v, v, 60.0 - v, 60.0 - v]);
    }
    deep.rect([0.0, 0.0, 64.0, 64.0], Color(0, 0, 0, 255));
    for _ in 0..10 {
        deep.pop_clip();
    }
    let mut a = Canvas::new(W, H);
    a.render(&deep, W, H);

    // The same list with only nine scopes: the tenth is the one the
    // stack refuses, so the two must agree.
    let mut eight = DisplayList::new();
    for k in 0..9u32 {
        let v = k as f32;
        eight.push_clip([v, v, 60.0 - v, 60.0 - v]);
    }
    eight.rect([0.0, 0.0, 64.0, 64.0], Color(0, 0, 0, 255));
    for _ in 0..9 {
        eight.pop_clip();
    }
    let mut b = Canvas::new(W, H);
    b.render(&eight, W, H);

    println!("ten scopes ink {} px, nine ink {} px", ink(&a), ink(&b));
    assert_eq!(
        ink(&a),
        ink(&b),
        "a ninth list scope changed what was drawn; it must be ignored, \
         which leaves the eight above it in force"
    );
    // And it really did clip: the last kept scope is 52x52 = 2704 px,
    // well under the canvas.
    assert!(ink(&a) > 0, "nothing was drawn at all — the fixture clipped everything away");
    assert!(
        ink(&a) <= 52 * 52,
        "{} px inked, more than the last kept scope's 2704 — the stack is \
         not clipping at depth",
        ink(&a)
    );
}

/// **Balance survives the refusal.** A ninth push and its matching pop
/// must leave the eighth scope in force, not pop it — otherwise the
/// command after the pop paints wider than the list asked.
#[test]
fn the_pop_that_matches_a_refused_push_pops_nothing() {
    let mut list = DisplayList::new();
    // Eight list scopes, ending in a 20x20 box.
    for k in 0..7u32 {
        let v = k as f32;
        list.push_clip([v, v, 60.0 - v, 60.0 - v]);
    }
    list.push_clip([10.0, 10.0, 30.0, 30.0]);
    // A ninth, refused, and popped again.
    list.push_clip([0.0, 0.0, 64.0, 64.0]);
    list.pop_clip();
    // This must still be bounded by the 20x20 scope.
    list.rect([0.0, 0.0, 64.0, 64.0], Color(0, 0, 0, 255));
    for _ in 0..8 {
        list.pop_clip();
    }
    let mut c = Canvas::new(W, H);
    c.render(&list, W, H);
    println!("after a refused push and its pop: {} px inked", ink(&c));
    assert_eq!(
        ink(&c),
        20 * 20,
        "the fill covered {} px, not the 400 of the scope that should still \
         be in force — the pop matching a refused push popped a real scope",
        ink(&c)
    );
}
