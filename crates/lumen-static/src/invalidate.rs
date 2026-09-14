//! What a frame knows about which subtrees changed —
//! `docs/plan-invalidation.md` T16.7.
//!
//! # The property this exists for
//!
//! The design spec's §2 ranks it first of the four that produce
//! performance: *"a mutation sets a dirty flag that propagates up only
//! until it reaches a relayout boundary… a text change deep in a list
//! costs O(depth-to-boundary), not O(tree)."*
//!
//! It was not built. [`crate::desc::NodeDesc::RIGID`] has been computed
//! by the derive since M8 and read by exactly one thing in the
//! workspace — a diagnostic counter in the agent crate — so every frame
//! lays out, places and paints every node whatever changed. Measured on
//! 2026-09-08: a one-row change in a 10 000-row list spends **176 µs of
//! a 231 µs frame** in layout + place + paint, 76% of it, for 9 999 rows
//! that did not change (`docs/results-competitive.md` §5b).
//!
//! # Why a type parameter, and why the whole-tree walk stays
//!
//! [`Everything`] is a ZST that reports every subtree dirty — today's
//! behaviour exactly — and it is **the oracle**, not a legacy path. This
//! crate has used that shape twice already: `Rebuild` is what
//! `Incremental` is tested against, and a full repaint is what
//! `damage_oracle.rs` compares a partial one to. A bounded walk that
//! cannot be checked against the full walk is a bounded walk nobody can
//! trust, so both live in one binary and a cargo feature is the wrong
//! mechanism (the reasoning M8b/T8b.0 recorded for retention).
//!
//! # Why subtree *ranges*, not a bit per node
//!
//! [`crate::NodeState`] is exactly full at 76 bytes — 72 of payload,
//! then `pressed`, `hovered`, `focused` and `focus_ring` filling the
//! last four — and `fixtures/embedded` asserts that against the RAM
//! budget in `docs/results-embedded.md`. A dirty bit would cost 4 bytes
//! on every node of every tree, including trees that never invalidate
//! anything.
//!
//! It is also unnecessary. The table is DFS pre-order, so **a subtree is
//! a contiguous index range** ([`crate::style::subtree_range`]) — the
//! same property that makes a scoped cascade a slice. "This subtree
//! changed" is therefore two `u32`s, and a set of them is a small array
//! with no allocator behind it.
//!
//! # What is *not* answered here
//!
//! How a leaf mutation reaches the set. `#[bind]` knows which tree field
//! it writes and can mark at zero runtime cost; a handler taking
//! `&mut App` can touch anything and cannot be narrowed by its dispatch
//! target without being wrong. The conservative answer — a handler that
//! ran marks everything — is what [`crate::event::App`]'s callers use,
//! and it degrades to today's behaviour rather than to a wrong frame.
//! Recorded as OPEN in `docs/plan-invalidation.md` T16.7.
//!
//! # Pointer and focus state is marked by the host
//!
//! A hover, a press, a release, a focus move and focus becoming visible
//! all change what a node paints without any handler running. The host
//! compares [`crate::event::InputState`]'s `hovered`, `pressed`, `focus`
//! and `focus_visible` across dispatch and marks the subtree of every
//! node whose state changed — the old one and the new one. Until
//! M25/T25.0 nothing did, and under [`Boundaries`] a hovered button kept
//! its resting shade: the derive skipped the clean subtree before the
//! shade was resolved. [`Everything`] lays out every node regardless,
//! which is why the oracle, whose session never hovered, did not see it
//! (`docs/plan-appearance.md` T25.0).

use core::ops::Range;

/// The **read** side of a frame's dirty set, borrowed.
///
/// [`Invalidation`] carries an associated const, which makes it not
/// dyn-compatible, and [`crate::cx::LayoutCx`] is deliberately concrete
/// — it threads `&mut dyn TextMeasure` rather than a type parameter,
/// because a type parameter there would be viral through every widget
/// signature and would spend §8's single-type-axis budget. So the query
/// side is this: a plain borrowed view with no trait and no vtable,
/// which a `LayoutCx` can hold and a widget can ask.
///
/// [`Changed::ALL`] is the conservative value and the default, so a
/// `LayoutCx` built the way every existing caller builds one behaves
/// exactly as it did before T16.7.
#[derive(Clone, Copy, Debug)]
pub struct Changed<'a> {
    ranges: &'a [(u32, u32)],
    all: bool,
}

impl Changed<'_> {
    /// Everything is dirty — what a caller that tracks nothing supplies,
    /// and what a set that has overflowed collapses to.
    pub const ALL: Changed<'static> = Changed { ranges: &[], all: true };

    /// Nothing is dirty. Only correct for a frame in which genuinely
    /// nothing was marked; `ALL` is the safe default.
    pub const NONE: Changed<'static> = Changed { ranges: &[], all: false };

    /// Is the subtree occupying `r` untouched this frame?
    ///
    /// `false` when unsure. The question is *intersection*, not
    /// containment: a mark on an ancestor dirties its descendants, and a
    /// mark on a descendant dirties every ancestor whose size may depend
    /// on it.
    #[inline]
    pub fn is_clean(&self, r: Range<usize>) -> bool {
        if self.all || r.start >= r.end {
            return false;
        }
        let (s, e) = (r.start as u32, r.end as u32);
        !self.ranges.iter().any(|&(rs, re)| rs < e && s < re)
    }

    /// Whether this view can ever answer `true`. A caller can hoist the
    /// check out of a loop rather than asking per child.
    #[inline]
    pub fn can_skip(&self) -> bool {
        !self.all
    }
}

/// A frame's dirty set, over subtree index ranges.
///
/// Implementors are cheap to default-construct and must be conservative
/// when in doubt: reporting a clean subtree dirty costs time, reporting
/// a dirty subtree clean costs correctness, and only one of those is
/// recoverable.
pub trait Invalidation: Default {
    /// Whether this implementation can ever report anything clean.
    ///
    /// `false` for [`Everything`], which lets a consumer compile the
    /// check away entirely rather than branch on a constant.
    const CAN_SKIP: bool;

    /// Mark the half-open index range `r` as changed.
    fn mark(&mut self, r: Range<usize>);

    /// Give up on precision: every subtree is dirty this frame.
    ///
    /// The answer to a resize, a stylesheet change, a structural swap,
    /// or anything else whose blast radius is not known — and to running
    /// out of room, which is why it is not an error.
    fn mark_all(&mut self);

    /// Is the subtree occupying `r` untouched this frame?
    ///
    /// Must return `false` when unsure. Note the question is about
    /// *intersection*, not containment: a mark on an ancestor dirties
    /// its descendants, and a mark on a descendant dirties the ancestor
    /// whose size may depend on it.
    fn is_clean(&self, r: Range<usize>) -> bool;

    /// Start a new frame with nothing marked.
    fn clear(&mut self);

    /// The borrowed read side, for threading through
    /// [`crate::cx::LayoutCx`].
    fn view(&self) -> Changed<'_>;
}

/// Everything is dirty, always — today's whole-tree walk, and the
/// oracle every bounded walk is checked against.
///
/// A ZST with a `const CAN_SKIP: bool = false`, so a consumer's
/// `if I::CAN_SKIP && …` compiles to nothing and a `Host<A, R,
/// Everything>` is the code this crate shipped before T16.7.
#[derive(Default, Clone, Copy, Debug)]
pub struct Everything;

impl Invalidation for Everything {
    const CAN_SKIP: bool = false;

    #[inline]
    fn mark(&mut self, _r: Range<usize>) {}
    #[inline]
    fn mark_all(&mut self) {}
    /// Never clean. Not "nothing was marked, so nothing changed" —
    /// this type's whole contract is that it does not track, and
    /// answering `true` here would turn "I don't know" into "I know it
    /// is fine", which is the one direction that loses frames.
    #[inline]
    fn is_clean(&self, _r: Range<usize>) -> bool {
        false
    }
    #[inline]
    fn clear(&mut self) {}
    #[inline]
    fn view(&self) -> Changed<'_> {
        Changed::ALL
    }
}

/// A bounded set of dirty subtree ranges.
///
/// `N` ranges, inline, no allocator — so this is available in the
/// `no_std` profile at both tiers. Overflowing `N` is not an error and
/// not a panic: the set collapses to "everything", which is slower and
/// still correct. That is the same refusal-rather-than-failure shape
/// [`crate::InputQueue`] uses when it fills.
///
/// Ranges are **merged on overlap** rather than accumulated, so a widget
/// marking itself and then its parent marking the same subtree costs one
/// slot, not two.
#[derive(Clone, Copy, Debug)]
pub struct Boundaries<const N: usize = 16> {
    /// `(start, end)` half-open, in table index space.
    ranges: [(u32, u32); N],
    len: u8,
    /// Set when a mark did not fit, or when a caller asked for it.
    all: bool,
}

impl<const N: usize> Default for Boundaries<N> {
    fn default() -> Self {
        Boundaries { ranges: [(0, 0); N], len: 0, all: true }
    }
}

impl<const N: usize> Boundaries<N> {
    /// How many ranges this can hold before collapsing to "everything".
    pub const CAP: usize = N;

    /// Ranges currently marked. `0` with [`Boundaries::is_all`] false
    /// means a genuinely still frame.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0 && !self.all
    }

    /// Whether the set has collapsed to "every subtree is dirty".
    ///
    /// Worth exposing: an application whose set collapses every frame is
    /// paying for the machinery and getting nothing, and the only way to
    /// find that out is to ask.
    pub fn is_all(&self) -> bool {
        self.all
    }
}

impl<const N: usize> Invalidation for Boundaries<N> {
    const CAN_SKIP: bool = true;

    fn mark(&mut self, r: Range<usize>) {
        if self.all || r.start >= r.end {
            return;
        }
        let (mut s, mut e) = (r.start as u32, r.end as u32);
        // Absorb every range this one touches, then store the union.
        // Two subtrees either nest or are disjoint, so a merged range is
        // still exactly a subtree — but the arithmetic does not depend
        // on that and stays correct for any range a caller invents.
        let mut w = 0;
        for k in 0..self.len as usize {
            let (rs, re) = self.ranges[k];
            if rs < e && s < re {
                s = s.min(rs);
                e = e.max(re);
            } else {
                self.ranges[w] = (rs, re);
                w += 1;
            }
        }
        if w >= N {
            self.mark_all();
            return;
        }
        self.ranges[w] = (s, e);
        self.len = (w + 1) as u8;
    }

    fn mark_all(&mut self) {
        self.all = true;
        self.len = 0;
    }

    fn is_clean(&self, r: Range<usize>) -> bool {
        if self.all || r.start >= r.end {
            return false;
        }
        let (s, e) = (r.start as u32, r.end as u32);
        !self.ranges[..self.len as usize].iter().any(|&(rs, re)| rs < e && s < re)
    }

    fn clear(&mut self) {
        self.len = 0;
        self.all = false;
    }

    fn view(&self) -> Changed<'_> {
        Changed { ranges: &self.ranges[..self.len as usize], all: self.all }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_is_never_clean_even_when_nothing_was_marked() {
        // The oracle's contract: it does not track, so it must not
        // claim to know. A `true` here would be "I don't know" read as
        // "it is fine", which is the direction that loses frames.
        let e = Everything;
        assert!(!e.is_clean(0..1));
        assert!(!e.is_clean(5..9));
        // A compile-time check, the way this crate states the others:
        // `CAN_SKIP` is a const, so `assert!` on it is a lint and a
        // `const _` is the assertion that actually cannot drift.
        const _: () = assert!(!Everything::CAN_SKIP);
    }

    #[test]
    fn a_fresh_set_is_all_dirty_until_a_frame_starts() {
        // Default is `all`, not empty: a set nobody has cleared has not
        // observed a frame, and the first frame must draw everything.
        let b: Boundaries<4> = Boundaries::default();
        assert!(b.is_all());
        assert!(!b.is_clean(0..1));
    }

    #[test]
    fn a_mark_dirties_itself_its_ancestors_and_its_descendants() {
        let mut b: Boundaries<4> = Boundaries::default();
        b.clear();
        // The subtree [4, 8) changed.
        b.mark(4..8);
        assert!(!b.is_clean(4..8), "itself");
        assert!(!b.is_clean(5..6), "a descendant");
        assert!(!b.is_clean(0..20), "an ancestor whose size may depend on it");
        assert!(b.is_clean(8..12), "a later sibling");
        assert!(b.is_clean(0..4), "an earlier sibling");
    }

    #[test]
    fn overlapping_marks_merge_instead_of_filling_the_set() {
        let mut b: Boundaries<2> = Boundaries::default();
        b.clear();
        b.mark(4..8);
        b.mark(6..10); // overlaps
        assert_eq!(b.len(), 1, "merged");
        assert!(!b.is_clean(4..10));
        assert!(b.is_clean(10..12));
    }

    #[test]
    fn overflowing_the_set_collapses_to_everything_rather_than_dropping_a_mark() {
        // Dropping a mark would be a wrong frame; collapsing is a slow
        // one. `InputQueue` refuses the same way when it fills.
        let mut b: Boundaries<2> = Boundaries::default();
        b.clear();
        b.mark(0..1);
        b.mark(4..5);
        assert_eq!(b.len(), 2);
        assert!(b.is_clean(8..9));
        b.mark(8..9);
        assert!(b.is_all(), "collapsed");
        assert!(!b.is_clean(100..101), "and nothing is clean any more");
    }

    #[test]
    fn clear_starts_a_still_frame() {
        let mut b: Boundaries<4> = Boundaries::default();
        b.clear();
        assert!(b.is_empty());
        assert!(b.is_clean(0..99), "nothing changed, so everything may be skipped");
        b.mark_all();
        assert!(!b.is_clean(0..99));
        b.clear();
        assert!(b.is_clean(0..99), "and clearing forgets it again");
    }

    #[test]
    fn the_view_answers_the_same_as_the_owner() {
        let mut b: Boundaries<4> = Boundaries::default();
        b.clear();
        b.mark(4..8);
        let v = b.view();
        for r in [0..4, 4..8, 5..6, 0..20, 8..12] {
            assert_eq!(v.is_clean(r.clone()), b.is_clean(r.clone()), "disagreed on {r:?}");
        }
        assert!(v.can_skip());
        assert!(!Everything.view().can_skip(), "the oracle can never skip");
    }

    #[test]
    fn an_empty_range_is_never_clean() {
        // A degenerate range is a caller mistake, not a licence to skip.
        let mut b: Boundaries<4> = Boundaries::default();
        b.clear();
        assert!(!b.is_clean(5..5));
        b.mark(5..5);
        assert!(b.is_empty(), "and marking one records nothing");
    }
}
