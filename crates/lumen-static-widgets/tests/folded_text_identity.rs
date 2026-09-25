//! M46/T46.0 item 2, R2-F3: a `FoldedText` memo serves the width of the
//! text it holds, not of a different string that folds to the same key.
//!
//! `Button`, `Badge` and `Chip` have no dirty bit — their text is a plain
//! field the application writes — so their memo keys on
//! `FoldedText::fold_of(text)`. Until T46.1 that was `display::fold` from
//! 0 over the bytes: linear, and with no length. So two kinds of pair
//! share a key, and each is here for a different reason:
//!
//! - **`"x"` → `"\0x"` and `"x"` → `"\0\0\0x"`**: a fold that starts at 0
//!   cannot see a leading zero byte. Any function seeded with the length
//!   separates these *structurally*, so they are the rows that would stay
//!   green for the right reason under a different fold.
//! - **the solved same-length pair**: `Σ dᵢ·K^(11-i) ≡ 0 (mod 2⁶⁴)` for
//!   today's `K`, solved by lattice reduction (M38/R2, probe 7cadfa6).
//!   It is the one input guaranteed to exercise the linear defect, and a
//!   length seed plus a finaliser does not separate it (R2-F4) — which is
//!   what the gate shows if that is swapped in as "the fix".
//!
//! Each row warms the widget on the first string, writes the second,
//! lays it out again, and compares the width served with a fresh widget's
//! width for the second string: `served w / correct w`. Same-length pairs
//! need a proportional measurer to have different widths at all, so they
//! run under [`Prop`]; the NUL rows run under `BlockMetrics` too.
//!
//! The control row, a same-length string that does not collide, is equal
//! before and after any fix: the gate accepting good input.

use lumen_static::cx::{BlockMetrics, TextMeasure};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Size};
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::{Badge, Chip};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 1000.0, min_h: 0.0, max_h: 200.0 };

/// Solved against `FoldedText`'s fold from 0 (the retention stream's pair
/// in `lumen-static-shell/tests/text_hash_collision.rs` is solved against
/// a different prefix, so it is a different pair).
const SOLVED_A: &str = "akaihaalsioa";
const SOLVED_B: &str = "eadaaenaaaai";
/// One byte off `SOLVED_B`, and the same width under [`Prop`]: a string
/// that does not collide, so the served width must be right today.
const CONTROL: &str = "eadaaenaaaaj";

/// A proportional measurer: `i`/`l`/`j`/space narrow, `m`/`w`/`$`/`#`
/// wide, as in any real face.
#[derive(Default)]
struct Prop;
impl TextMeasure for Prop {
    fn measure(&mut self, text: &str, size: f32, _max_w: f32) -> Size {
        let w: f32 = text
            .chars()
            .map(|c| match c {
                'i' | 'l' | 'j' | ' ' => 0.25,
                'm' | 'w' | '$' | '#' => 0.8,
                _ => 0.5,
            })
            .sum();
        Size { w: w * size, h: size }
    }
}

fn lay<W: Widget + Node>(w: &mut W, m: &mut dyn TextMeasure) -> Size {
    let styles = vec![<W as Node>::defaults(w); <W as Widget>::NODE_COUNT];
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let mut cx = lumen_static::LayoutCx::new(m);
    Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0)
}

#[derive(Clone, Copy)]
enum Kind {
    Button,
    Badge,
    Chip,
}

/// `(served, correct)` for `kind` warmed on `a` and then shown `b`.
fn served(kind: Kind, m: &mut dyn TextMeasure, a: &str, b: &str) -> (f32, f32) {
    match kind {
        Kind::Button => {
            let mut w: Button<(), String> = Button::default();
            w.label = a.into();
            lay(&mut w, m);
            w.label = b.into();
            let served = lay(&mut w, m).w;
            let mut fresh: Button<(), String> = Button::default();
            fresh.label = b.into();
            (served, lay(&mut fresh, m).w)
        }
        Kind::Badge => {
            let mut w: Badge<String> = Badge::new(a.into());
            lay(&mut w, m);
            w.set(b.into());
            let served = lay(&mut w, m).w;
            (served, lay(&mut Badge::<String>::new(b.into()), m).w)
        }
        Kind::Chip => {
            let mut w: Chip<String> = Chip::new(a.into());
            lay(&mut w, m);
            w.set(b.into());
            let served = lay(&mut w, m).w;
            (served, lay(&mut Chip::<String>::new(b.into()), m).w)
        }
    }
}

type Row = (&'static str, fn() -> Box<dyn TextMeasure>, &'static str, &'static str);

fn block() -> Box<dyn TextMeasure> {
    Box::new(BlockMetrics)
}
fn prop() -> Box<dyn TextMeasure> {
    Box::new(Prop)
}

/// The colliding rows: every one of them is red at HEAD before T46.1.
const COLLIDING: &[Row] = &[
    ("BlockMetrics", block, "x", "\u{0}x"),
    ("BlockMetrics", block, "x", "\u{0}\u{0}\u{0}x"),
    ("Prop", prop, "x", "\u{0}x"),
    ("Prop", prop, SOLVED_A, SOLVED_B),
];

fn check(kind: Kind, name: &str, rows: &[Row]) {
    let mut wrong = Vec::new();
    for &(mname, m, a, b) in rows {
        let (s, c) = served(kind, &mut *m(), a, b);
        let ok = s == c;
        println!("[{mname}] {name} {a:?} -> {b:?}: served w {s} / correct w {c}  {}", if ok { "==" } else { "STALE" });
        if !ok {
            wrong.push(format!("{mname} {a:?}->{b:?}: {s} vs {c}"));
        }
    }
    assert!(wrong.is_empty(), "{name} served a stale width: {wrong:?}");
}

#[test]
#[ignore = "red until T46.1: R2-F3, FoldedText keys on a linear, length-free fold"]
fn button_serves_the_width_of_the_text_it_holds() {
    check(Kind::Button, "Button", COLLIDING);
}

#[test]
#[ignore = "red until T46.1: R2-F3, FoldedText keys on a linear, length-free fold"]
fn badge_serves_the_width_of_the_text_it_holds() {
    check(Kind::Badge, "Badge", COLLIDING);
}

#[test]
#[ignore = "red until T46.1: R2-F3, FoldedText keys on a linear, length-free fold"]
fn chip_serves_the_width_of_the_text_it_holds() {
    check(Kind::Chip, "Chip", COLLIDING);
}

/// Accepts good input: a change that does not collide is served right
/// by all three widgets, before and after the fix. It asserts that the
/// widths differ from the warm string's, so a row that could not have
/// shown a stale width does not count as passing.
#[test]
fn a_non_colliding_change_is_served_right() {
    for (kind, name) in [(Kind::Button, "Button"), (Kind::Badge, "Badge"), (Kind::Chip, "Chip")] {
        let (warm, _) = served(kind, &mut Prop, SOLVED_A, SOLVED_A);
        let (s, c) = served(kind, &mut Prop, SOLVED_A, CONTROL);
        println!("[Prop(control)] {name} {SOLVED_A:?} -> {CONTROL:?}: served w {s} / correct w {c} (warm w {warm})");
        assert_ne!(warm, c, "setup: the control must change the width, or a stale memo would pass");
        assert_eq!(s, c, "{name} served a stale width on a non-colliding change");
    }
}
