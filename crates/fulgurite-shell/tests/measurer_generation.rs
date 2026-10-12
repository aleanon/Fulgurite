//! **A text memo keys on which measurer answered** — M45/T45.2, the
//! measurer-identity half M41/T41.5 handed on (R2-F1's defence in depth,
//! `docs/plan-text-correctness.md` §2.3).
//!
//! `FontMetrics::set_stack` gives the measurer a new
//! `TextMeasure::generation`, and a warm `TextInput` re-shapes against it;
//! an unchanged measurer still hits. The counting wrapper forwards
//! `generation`, as `TextMeasure`'s rustdoc says a wrapper must.
#![cfg(feature = "truetype")]

use std::cell::Cell;

use fulgurite::cx::GlyphSink;
use fulgurite::{Constraints, LayoutCx, Node, NodeState, Size, Style, TextMeasure};
use fulgurite_shell::text::{FontMetrics, FontStack};
use fulgurite_widgets::TextInput;

#[path = "fonts/pinned.rs"]
mod pinned;

struct Counting {
    inner: FontMetrics,
    calls: Cell<u32>,
}

impl TextMeasure for Counting {
    fn measure(&mut self, t: &str, size: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        self.inner.measure(t, size, max_w)
    }
    fn measure_weighted(&mut self, t: &str, size: f32, weight: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        self.inner.measure_weighted(t, size, weight, max_w)
    }
    fn shape(&mut self, t: &str, size: f32, weight: f32, max_w: f32, sink: &mut dyn GlyphSink) -> Size {
        self.calls.set(self.calls.get() + 1);
        self.inner.shape(t, size, weight, max_w, sink)
    }
    fn line_height(&mut self, size: f32) -> f32 {
        self.inner.line_height(size)
    }
    fn generation(&self) -> u32 {
        self.inner.generation()
    }
}

const LIMITS: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

fn layout(w: &mut TextInput<String>, m: &mut Counting) {
    let mut cx = LayoutCx::new(m);
    let (sty, mut st) = ([Style::default()], [NodeState::default()]);
    w.layout(&mut cx, LIMITS, &sty, &mut st, 0);
}

#[test]
fn a_new_generation_reshapes_a_warm_field() {
    let stack = FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
    let mut m = Counting { inner: FontMetrics::new(stack.clone()), calls: Cell::new(0) };
    let mut w = TextInput::<String>::new("hello");
    layout(&mut w, &mut m);
    layout(&mut w, &mut m);
    let warm = m.calls.get();
    let g0 = m.generation();
    m.inner.set_stack(stack);
    layout(&mut w, &mut m);
    let after = m.calls.get();
    println!("generation {g0} -> {}: calls {warm} before, {after} after (want 1 cold call, then at least one more)", m.generation());
    assert_ne!(g0, 0, "a real stack answers a generation a stand-in cannot");
    assert_eq!(warm, 1, "the second layout under the same measurer re-shaped");
    assert!(after > warm, "a new generation was served the old stack's shaping");
}
