//! `#[widget(..., fuse)]` support — handoff §5's erasure dial.
//!
//! A fused struct is one table entry and one kind; its subtree's layout
//! and paint are straight-line monomorphized code via [`Inline`], which
//! children are called through *directly* (no fn pointers), so the
//! compiler inlines and const-folds across the whole subtree — the same
//! trade as kernel fusion in an ML compiler, chosen locally at the
//! component seam instead of globally (§9).
//!
//! What fusion costs, by design:
//! - internal geometry is **recomputed** in paint from the node's
//!   constraints (a fused subtree has no per-node `NodeState`); the bet,
//!   measured in `docs/results-step6-fuse.md`, is that straight-line
//!   recompute beats streamed state plus dispatch at small sizes;
//! - internals are invisible to observability — one AccessKit node, one
//!   provenance entry (the `extra` mechanism the repeater uses could
//!   re-expose them; deferred, see the step doc);
//! - monomorphization (§4's compile-time/code-size trade) is back on,
//!   but bounded to the subtree that opted in.
//!
//! Fusion is inward-only: fused widgets sit anywhere a widget can, but
//! their fields must be `Inline` (leaves or other fused structs) — a
//! table container inside a fused one is a compile error, on purpose.

use crate::desc::{Constraints, Point, Size};
use crate::ops::{fold, Fixed, Label, Spacer};

/// The monomorphized layout/paint path for fused subtrees.
pub trait Inline {
    /// Size under constraints. Straight-line; no dispatch.
    fn measure(&self, c: Constraints) -> Size;
    /// Fold paint output, given the constraints this widget was measured
    /// under, its absolute origin, and the size `measure` returned.
    fn paint_inline(&self, c: Constraints, pos: Point, size: Size, out: &mut u64);
}

/// Checksum helper for fused paint. `tag` stands in for the table-local
/// kind index fused nodes don't have (0 container, 1 label, 2 fixed,
/// 3 spacer); fused and unfused checksums are not comparable, geometry is.
pub fn fold_tagged(out: &mut u64, tag: u64, size: Size, pos: Point) {
    fold(out, tag);
    fold(out, size.w.to_bits() as u64 ^ ((size.h.to_bits() as u64) << 32));
    fold(out, pos.x.to_bits() as u64 ^ ((pos.y.to_bits() as u64) << 32));
}

// Leaf impls mirror the table ops' math exactly (equivalence is tested).

impl Inline for Label {
    #[inline]
    fn measure(&self, c: Constraints) -> Size {
        c.clamp(Size { w: 8.0 * self.text.len() as f32, h: 16.0 })
    }
    #[inline]
    fn paint_inline(&self, _c: Constraints, pos: Point, size: Size, out: &mut u64) {
        fold_tagged(out, 1, size, pos);
        for &byte in self.text.as_bytes() {
            fold(out, byte as u64);
        }
    }
}

impl Inline for Fixed {
    #[inline]
    fn measure(&self, c: Constraints) -> Size {
        c.clamp(Size { w: self.w, h: self.h })
    }
    #[inline]
    fn paint_inline(&self, _c: Constraints, pos: Point, size: Size, out: &mut u64) {
        fold_tagged(out, 2, size, pos);
        fold(out, self.w.to_bits() as u64 ^ self.h.to_bits() as u64);
    }
}

impl Inline for Spacer {
    #[inline]
    fn measure(&self, c: Constraints) -> Size {
        Size { w: c.max_w, h: c.max_h }
    }
    #[inline]
    fn paint_inline(&self, _c: Constraints, pos: Point, size: Size, out: &mut u64) {
        fold_tagged(out, 3, size, pos);
    }
}
