//! M10a: `Painter` is the seam `paint` writes through.
//!
//! The property worth pinning is not that `DisplayList` implements it —
//! that is trivial — but that a *non-collecting* implementation sees
//! exactly what a collecting one records, including the text bytes it
//! cannot hold a range into.

use fulgurite::display::{Color, DisplayList, Painter};

const RED: Color = Color(255, 0, 0, 255);
const R: [f32; 4] = [1.0, 2.0, 30.0, 40.0];

/// Records what it was told, keeping text inline — what an
/// immediate-mode host does instead of interning.
#[derive(Default)]
struct Echo {
    calls: Vec<String>,
}

impl Painter for Echo {
    // M82/T82.3: a leaf with no fill table.
    fn fills(&self) -> &[fulgurite::style::Fill] {
        &[]
    }
    fn rect(&mut self, r: [f32; 4], c: Color) {
        self.calls.push(format!("rect {r:?} {c:?}"));
    }
    fn text_spaced(&mut self, r: [f32; 4], s: &str, size: f32, w: f32, lh: u8, c: Color) {
        self.calls.push(format!("text {r:?} {s:?} {size} {w} {lh} {c:?}"));
    }
    fn rounded_rect(&mut self, r: [f32; 4], c: Color, radius: f32) {
        self.calls.push(format!("round {r:?} {c:?} {radius}"));
    }
    fn shadow(&mut self, r: [f32; 4], radius: f32, spread: f32) {
        self.calls.push(format!("shadow {r:?} {radius} {spread}"));
    }
    fn stroked_rect(&mut self, r: [f32; 4], c: Color, radius: f32, width: f32) {
        self.calls.push(format!("stroke {r:?} {c:?} {radius} {width}"));
    }
    fn push_clip(&mut self, r: [f32; 4]) {
        self.calls.push(format!("push {r:?}"));
    }
    fn pop_clip(&mut self) {
        self.calls.push("pop".into());
    }
    fn path(&mut self, els: &[fulgurite::display::PathEl], xf: fulgurite::display::Affine, paint: fulgurite::display::PathPaint) {
        self.calls.push(format!("path {els:?} {xf:?} {paint:?}"));
    }
}

fn draw(p: &mut dyn Painter) {
    p.push_clip(R);
    p.rect(R, RED);
    p.rounded_rect(R, RED, 4.0);
    p.shadow(R, 4.0, 8.0);
    p.text(R, "hi", 16.0, RED);
    p.text_weighted(R, "bold", 16.0, 700.0, RED);
    p.pop_clip();
}

#[test]
fn a_non_collecting_painter_sees_every_command() {
    let mut e = Echo::default();
    draw(&mut e);
    assert_eq!(e.calls.len(), 7, "{:#?}", e.calls);
    assert!(e.calls[4].contains("\"hi\""), "text arrives as bytes: {}", e.calls[4]);
    assert!(e.calls[5].contains("700"), "and carries its weight");
}

#[test]
fn the_default_text_method_is_the_weighted_one_at_400() {
    // `text` is provided on the trait so an implementor writes one
    // method, not two — but it must not lose the weight.
    let mut e = Echo::default();
    e.text(R, "x", 16.0, RED);
    assert!(e.calls[0].starts_with("text"), "{:?}", e.calls);
    assert!(e.calls[0].contains(" 400 "), "default weight is 400: {}", e.calls[0]);
}

#[test]
fn collecting_through_the_trait_equals_collecting_directly() {
    // The forwarding impl must not change what a DisplayList records —
    // this is what makes T10a.2's signature change behaviour-preserving.
    let mut through = DisplayList::new();
    draw(&mut through);

    let mut direct = DisplayList::new();
    direct.push_clip(R);
    direct.rect(R, RED);
    direct.rounded_rect(R, RED, 4.0);
    direct.shadow(R, 4.0, 8.0);
    direct.text(R, "hi", 16.0, RED);
    direct.text_weighted(R, "bold", 16.0, 700.0, RED);
    direct.pop_clip();

    assert_eq!(through.cmds, direct.cmds);
    assert_eq!(through.text, direct.text, "and the interned bytes agree");
}

#[test]
fn a_display_list_still_signs_command_ranges_through_the_trait() {
    // What the change journal does: note where a node started, paint,
    // hash from there. It has to keep working when `paint` is handed a
    // `&mut dyn Painter` — which it does, because unsized coercion is
    // call-site-local and leaves the caller's own type concrete.
    let mut list = DisplayList::new();
    list.rect(R, RED);
    let from = list.len();
    draw(&mut list);
    let sig = list.hash_from(from);

    let mut again = DisplayList::new();
    again.rect(R, RED);
    let from2 = again.len();
    draw(&mut again);
    assert_eq!(sig, again.hash_from(from2), "the same paint signs the same");
    assert_ne!(sig, list.hash_from(0), "and the range is honoured");
}
