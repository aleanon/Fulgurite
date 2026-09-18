//! M27/T27.1 — the border, at the pixel.
//!
//! `crates/lumen-static/tests/border.rs` asserts the *command* is
//! recorded. That is the half a command-list test can see, and it passes
//! on a backend that draws nothing — which is the "did it paint" trap
//! `docs/plan-widgets-gallery.md` §3b is a table of.
//!
//! So this renders one and reads the buffer. Three claims a command list
//! cannot make:
//!
//! 1. the border is a **ring**, not a filled box — the interior keeps
//!    the fill it was given;
//! 2. it is **inset** — the pixel just outside the box is untouched,
//!    which is the difference between a border and `ops::focus_ring`'s
//!    deliberately-outset underlay;
//! 3. it is **the thickness asked for**, measured by counting inked rows
//!    rather than trusting the command.

use lumen_static::desc::{NodeState, Point, Size};
use lumen_static::display::{Color, DisplayList};
use lumen_static_shell::software::Canvas;

const W: u32 = 64;
const H: u32 = 64;
const CLEAR: Color = Color(0, 0, 0, 255);
const FILL: Color = Color(255, 255, 255, 255);
const INK: Color = Color(255, 0, 0, 255);
/// A whole box, well inside the canvas so "just outside" is a real pixel.
const BOX: [f32; 4] = [16.0, 16.0, 48.0, 48.0];
const BW: f32 = 3.0;

fn render(cmds: impl FnOnce(&mut DisplayList)) -> Canvas {
    let mut list = DisplayList::new();
    cmds(&mut list);
    let mut c = Canvas::new(W, H);
    c.clear(CLEAR);
    <Canvas as lumen_static::Renderer>::render(&mut c, &list, W, H);
    c
}

fn px(c: &Canvas, x: u32, y: u32) -> (u8, u8, u8) {
    let i = ((y * c.width + x) * 4) as usize;
    // The canvas is 0RGB / BGRA-ish depending on the platform word
    // order; compare channels as a set rather than assuming an order,
    // since every colour here is a pure primary or a grey.
    (c.pixels[i], c.pixels[i + 1], c.pixels[i + 2])
}

/// Is this pixel the red border rather than white fill or black ground?
fn is_ink(c: &Canvas, x: u32, y: u32) -> bool {
    let (a, b, d) = px(c, x, y);
    let mx = a.max(b).max(d);
    let mn = a.min(b).min(d);
    // Red on white/black is the only chromatic colour in the frame, so
    // "one channel much larger than another" identifies it without
    // depending on byte order.
    mx as i32 - mn as i32 > 60
}

fn is_fill(c: &Canvas, x: u32, y: u32) -> bool {
    let (a, b, d) = px(c, x, y);
    a > 200 && b > 200 && d > 200
}

fn is_clear(c: &Canvas, x: u32, y: u32) -> bool {
    let (a, b, d) = px(c, x, y);
    a < 40 && b < 40 && d < 40
}

/// **Through `ops::border`, not straight to the command.**
///
/// An injection showed why. Drawing `Cmd::StrokedRect` directly makes
/// `nothing_is_painted_outside_the_box` unfailable: the backend clips
/// every command to `clipped(rect, clip)`, which is a subset of the
/// rect, so no command can paint outside its own rect however the ring
/// is computed. The property is real but it lives one level up — in
/// whether the *widget* hands over its own box or an expanded one, which
/// is exactly the choice `ops::focus_ring` makes the other way. So the
/// test drives the helper every bordered widget calls.
fn bordered() -> Canvas {
    render(|l| {
        l.rounded_rect(BOX, FILL, 6.0);
        let mut st = [NodeState::default()];
        st[0].pos = Point { x: BOX[0], y: BOX[1] };
        st[0].size = Size { w: BOX[2] - BOX[0], h: BOX[3] - BOX[1] };
        st[0].radius = 6.0;
        st[0].border_color = INK;
        st[0].border_width = BW as u8;
        lumen_static::ops::border(&st, 0, l);
    })
}

/// (1) A ring, not a filled box.
///
/// **The centre alone is not enough, and an injection proved it.**
/// `stroke_rounded_within` skips the middle columns of any row between
/// the inner rect's corner bands — a real optimisation, since only the
/// two vertical edges can be covered there. So an injection that made
/// coverage the *outer* curve alone (a filled box) left the centre
/// white and this test green. The sample at `(32, 20)` is the fix: it
/// is inside the box, below the 3 px top edge, and in a row the scan
/// covers end to end.
#[test]
fn the_interior_keeps_its_fill() {
    let c = bordered();
    for (x, y, why) in [
        (32, 32, "the centre"),
        // In the corner-band rows, which are scanned across their whole
        // width — where a fill-instead-of-ring bug is actually visible.
        (32, 20, "just inside the top edge"),
        (32, 44, "just inside the bottom edge"),
    ] {
        assert!(
            is_fill(&c, x, y),
            "{why} at ({x},{y}) is {:?}, not the fill — the border filled the box              instead of ringing it",
            px(&c, x, y)
        );
    }
    // The edge itself is inked, or everything above proves only that
    // nothing was drawn at all.
    assert!(is_ink(&c, 32, 17), "the top edge at y=17 is {:?}, not the border", px(&c, 32, 17));
}

/// (2) Inset: nothing is painted outside the node's own box.
#[test]
fn nothing_is_painted_outside_the_box() {
    let c = bordered();
    for (x, y, where_) in
        [(32, 14, "above"), (32, 49, "below"), (14, 32, "left"), (49, 32, "right")]
    {
        assert!(
            is_clear(&c, x, y),
            "{where_} the box at ({x},{y}) is {:?} — a border drawn outside would widen \
             every bordered control by {BW} px a side (`ops::focus_ring` does exactly that, \
             deliberately, which is what makes this a real choice and not a typo)",
            px(&c, x, y)
        );
    }
}

/// (3) The thickness asked for, counted rather than trusted.
#[test]
fn the_border_is_as_thick_as_it_was_asked_to_be() {
    let c = bordered();
    let x = 32;
    let inked = (0..H).filter(|&y| is_ink(&c, x, y)).count();
    // Two edges, top and bottom, at BW each. Printed beside the bound:
    // a filled box would read 32 here and a missing border 0, and both
    // are within a "greater than zero" check.
    assert_eq!(
        inked, (BW as usize) * 2,
        "counted {inked} inked rows down the middle; {BW} px of border top and bottom is {}, \
         a filled box would be {}",
        (BW as usize) * 2,
        (BOX[3] - BOX[1]) as usize
    );
}

/// And the negative: no command, no ink.
#[test]
fn no_stroke_command_leaves_the_fill_alone() {
    let c = render(|l| l.rounded_rect(BOX, FILL, 6.0));
    assert!(!is_ink(&c, 32, 17), "something drew a border nobody asked for");
    assert!(is_fill(&c, 32, 17), "the fill itself is missing, so this test proves nothing");
}
