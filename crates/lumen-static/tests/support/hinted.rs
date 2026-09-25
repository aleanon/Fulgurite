//! A measurer whose answer is **non-linear** in the display scale, the
//! way a hinted face's is — shared by `crates/lumen-static/tests/measure_scale.rs`
//! (the memo, at `LayoutCx` level) and `crates/lumen-static-shell/tests/scale_memo.rs`
//! (the same memo reached through `Host::pump_with`).
//!
//! `w = len * size + scale`: the `+ scale` term stands in for hinting.
//! It is deliberately not a multiple of `scale`, because a measurer
//! whose answer scaled linearly would report the same logical width at
//! every scale and could not exhibit a memo keyed without the scale at
//! all — the vacuous shape `AGENT.md` names first. `BlockMetrics` is
//! exactly that measurer, which is why these tests bring their own.
//!
//! Not a test target: Cargo builds only `tests/*.rs` and `tests/*/main.rs`,
//! so a file in `tests/support/` is compiled only where a test includes it.

use std::cell::Cell;

use lumen_static::{Size, TextMeasure};

#[derive(Default)]
pub struct Hinted {
    pub scale: f32,
    pub calls: Cell<usize>,
}

impl Hinted {
    pub fn at(scale: f32) -> Self {
        Hinted { scale, ..Hinted::default() }
    }
}

impl TextMeasure for Hinted {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.measure_weighted(text, size, 400.0, max_w)
    }
    fn measure_weighted(&mut self, text: &str, size: f32, _weight: f32, _max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        Size { w: text.len() as f32 * size + self.scale, h: size }
    }
}
