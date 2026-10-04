//! Content that paints on top of everything and is hit before it —
//! `docs/plan-capabilities.md` T15.0.
//!
//! # Why this is a phase and not a layout mode
//!
//! Two properties of the traversal defeat a widget-local overlay, and
//! neither is fixable inside a widget:
//!
//! - **A widget cannot escape an ancestor's clip.** `ScrollView::paint`
//!   pushes a clip, paints its content and pops it; a popup *inside*
//!   that subtree paints between those calls and is clipped by them. A
//!   dropdown inside a scrollable list is the commonest overlay there
//!   is, and it is exactly the case that fails.
//! - **A widget cannot paint above its later siblings.** Paint is a
//!   pre-order walk in declaration order, so a popup on child 2 is drawn
//!   before children 3..n and renders underneath them.
//!
//! A z-order layout mode fixes both and costs too much: it breaks the
//! invariant that **paint order is tree order**, which retention's
//! positional diff (T13.8) and the agent journal's per-node command
//! ranges both rest on, and it needs a node to have two parents in a
//! model whose premise is that the parent is a `const`.
//!
//! So **layout does not change at all**. [`pass::paint_overlaid`] runs
//! the ordinary walk, then repaints deferred subtrees *outside every
//! clip scope*. The stream is phase 1 then phase 2, both in table
//! order, so it stays deterministic and retention still holds. Opening
//! an overlay changes the stream's *length*, which `Incremental`
//! already answers with `Damage::All` — correct, and already built.
//!
//! [`pass::paint_overlaid`]: crate::pass::paint_overlaid
//!
//! # What it costs when nothing uses it
//!
//! Nothing. [`NoOverlays`] is a ZST whose [`OverlaySink::defer`] returns
//! `false`, so a widget that offers to defer paints inline instead and
//! the loop below never runs. That is the gating policy's second
//! mechanism — a type parameter, not a feature — and it is why this is
//! `no_std` with no allocator: [`Overlays`] is a fixed array.

/// How deep overlays may nest before the phase gives up.
///
/// A submenu inside a menu inside a popup is three. Deferrals made
/// *during* phase 2 are painted in the following round, so a chain
/// works; the cap stops a widget that defers itself from spinning.
pub const MAX_ROUNDS: usize = 4;

/// Where a deferred subtree is recorded until the overlay phase.
///
/// Implemented twice: [`NoOverlays`] refuses every deferral and costs
/// nothing, [`Overlays<N>`] holds up to `N` in a fixed array.
pub trait OverlaySink {
    /// Record `node` to be painted in the overlay phase.
    ///
    /// **Returns whether it was recorded.** A widget that gets `false`
    /// must paint inline instead — that is what makes `NoOverlays` a
    /// working configuration rather than a broken one, and what makes a
    /// full [`Overlays<N>`] degrade to a clipped popup rather than a
    /// missing one.
    fn defer(&mut self, node: u32) -> bool;

    /// How many deferrals are recorded.
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The `k`th deferral, in the order they were made.
    fn at(&self, k: usize) -> u32;

    /// Forget them all. `paint_overlaid` calls this before phase 1, so
    /// a sink is reusable across frames without the host clearing it.
    fn clear(&mut self);

    /// How many deferrals were refused for lack of room since the last
    /// [`clear`](Self::clear).
    ///
    /// Non-zero means an overlay was painted inline — clipped, and
    /// possibly under a later sibling. Counted rather than silent, on
    /// the same rule `InputQueue::dropped` follows.
    fn refused(&self) -> u32 {
        0
    }
}

/// No overlays: every deferral is refused and the phase never runs.
///
/// The default, and a ZST. A tree that declares no overlay compiles to
/// exactly the code it did before T15.0.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoOverlays;

impl OverlaySink for NoOverlays {
    fn defer(&mut self, _: u32) -> bool {
        false
    }
    fn len(&self) -> usize {
        0
    }
    fn at(&self, _: usize) -> u32 {
        // Unreachable through `paint_overlaid`, which guards on `len`.
        // A panic would be a worse answer than a harmless index for a
        // frame loop, so this returns the root.
        0
    }
    fn clear(&mut self) {}
}

/// Up to `N` deferred subtrees, in a fixed array.
///
/// `N` overlays cost `N * 4` bytes and no heap: `Overlays<8>` is 40
/// bytes, which is what a menu with submenus needs and less than one
/// `NodeState`. Size it above the deepest stack of simultaneously-open
/// overlays, not at the number of overlay *widgets* in the tree.
#[derive(Clone, Copy, Debug)]
pub struct Overlays<const N: usize> {
    buf: [u32; N],
    len: usize,
    refused: u32,
}

impl<const N: usize> Default for Overlays<N> {
    fn default() -> Self {
        Overlays { buf: [0; N], len: 0, refused: 0 }
    }
}

impl<const N: usize> OverlaySink for Overlays<N> {
    fn defer(&mut self, node: u32) -> bool {
        if self.len == N {
            self.refused = self.refused.saturating_add(1);
            return false;
        }
        self.buf[self.len] = node;
        self.len += 1;
        true
    }
    fn len(&self) -> usize {
        self.len
    }
    fn at(&self, k: usize) -> u32 {
        self.buf[k.min(N.saturating_sub(1))]
    }
    fn clear(&mut self) {
        self.len = 0;
        self.refused = 0;
    }
    fn refused(&self) -> u32 {
        self.refused
    }
}

/// A [`Painter`](crate::display::Painter) that routes deferrals to a
/// sink and forwards everything else.
///
/// The wrapper shape `Scaled` and `Recording` already use, and it
/// composes with them: `paint → Deferring(Recording(Scaled(sink)))`.
pub struct Deferring<'a, O: OverlaySink> {
    inner: &'a mut dyn crate::display::Painter,
    sink: &'a mut O,
    /// The node the overlay phase is currently painting, if any.
    ///
    /// Its own deferral is refused, which is what stops a widget that
    /// defers itself from being re-deferred forever: in phase 1 it is
    /// recorded, and in phase 2 it is told `false` and paints inline —
    /// which is correct there, because phase 2 already runs outside
    /// every clip.
    painting: Option<u32>,
}

impl<'a, O: OverlaySink> Deferring<'a, O> {
    /// Phase 1: nothing is being repainted, so every deferral is taken.
    pub fn new(inner: &'a mut dyn crate::display::Painter, sink: &'a mut O) -> Self {
        Deferring { inner, sink, painting: None }
    }

    /// Phase 2, repainting node `i`. `i`'s own deferral is refused; a
    /// *different* node deferred from inside it — a submenu — is taken
    /// and painted in the next round.
    pub fn repainting(
        inner: &'a mut dyn crate::display::Painter,
        sink: &'a mut O,
        i: u32,
    ) -> Self {
        Deferring { inner, sink, painting: Some(i) }
    }
}

impl<O: OverlaySink> crate::display::Painter for Deferring<'_, O> {
    fn rect(&mut self, rect: crate::display::Rect, color: crate::display::Color) {
        self.inner.rect(rect, color);
    }
    /// The one text forward: `text_weighted` and `text` reach it through
    /// their provided bodies (M80/T80.1), and it forwards the line pitch
    /// through `inner.text_spaced`, never `inner.text_weighted`, which
    /// would drop it.
    fn text_spaced(
        &mut self,
        rect: crate::display::Rect,
        s: &str,
        size: f32,
        weight: f32,
        line_height: u8,
        color: crate::display::Color,
    ) {
        self.inner.text_spaced(rect, s, size, weight, line_height, color);
    }
    fn rounded_rect(
        &mut self,
        rect: crate::display::Rect,
        color: crate::display::Color,
        radius: f32,
    ) {
        self.inner.rounded_rect(rect, color, radius);
    }

    fn stroked_rect(
        &mut self,
        rect: crate::display::Rect,
        color: crate::display::Color,
        radius: f32,
        width: f32,
    ) {
        self.inner.stroked_rect(rect, color, radius, width);
    }
    fn shadow(&mut self, rect: crate::display::Rect, radius: f32, spread: f32) {
        self.inner.shadow(rect, radius, spread);
    }
    // The four below are `Painter`'s *provided* methods, and forwarding
    // them is not optional -- **it was omitted, and the default bodies
    // silently rewrote the frame.** `line` and `image` default to no-ops
    // and `gradient` defaults to a flat fill of its first stop, so every
    // line and image painted under an overlay pass was dropped and every
    // gradient was flattened. Measured before the fix on a four-command
    // list: four commands in, two out.
    //
    // A provided body is the right answer for a *leaf* painter that
    // cannot draw the thing -- that is what `display.rs` says and it is
    // correct there. For a *wrapper* it is the wrong answer in every
    // case, because the wrapper can always forward, and the default
    // instead acts on the wrapper itself. `Scaled` and `Recording`
    // override all eleven; this type overrode seven, and the four it
    // missed are exactly the ones added after it (T15.2's image, T15.3's
    // line and gradient, T16.12's `clip_bounds`).
    //
    // `tests/overlay.rs::deferring_forwards_every_command` pins it.
    fn line(
        &mut self,
        a: crate::desc::Point,
        b: crate::desc::Point,
        width: f32,
        color: crate::display::Color,
    ) {
        self.inner.line(a, b, width, color);
    }
    fn gradient(
        &mut self,
        rect: crate::display::Rect,
        stops: crate::display::Stops,
        shape: crate::display::GradShape,
        corner: f32,
    ) {
        self.inner.gradient(rect, stops, shape, corner);
    }
    fn image(
        &mut self,
        rect: crate::display::Rect,
        id: crate::display::ImageId,
        tint: crate::display::Color,
    ) {
        self.inner.image(rect, id, tint);
    }
    /// Culling must survive an overlay pass. The default is `None`,
    /// which means "cull nothing" -- correct for a painter with no
    /// surface, and a silent 8x regression in `List::paint` here
    /// (T16.12, `docs/plan-invalidation.md`).
    fn clip_bounds(&self) -> Option<crate::display::Rect> {
        self.inner.clip_bounds()
    }
    /// Forwarded, so a deferred node's fill paints in phase 2 as it
    /// would have inline — M82/T82.3 (`tests/overlay.rs`).
    fn fills(&self) -> &[crate::style::Fill] {
        self.inner.fills()
    }
    /// Forwarded, so a deferred node's paths paint in phase 2 — M82/T82.5.
    /// The transform is the path's own argument, so there is no scope to
    /// balance across the phases.
    fn path(
        &mut self,
        els: &[crate::display::PathEl],
        xf: crate::display::Affine,
        paint: crate::display::PathPaint,
    ) {
        self.inner.path(els, xf, paint);
    }
    fn glass(
        &mut self,
        rect: crate::display::Rect,
        radius: f32,
        material: crate::display::Material,
    ) {
        self.inner.glass(rect, radius, material);
    }
    fn push_clip(&mut self, rect: crate::display::Rect) {
        self.inner.push_clip(rect);
    }
    fn pop_clip(&mut self) {
        self.inner.pop_clip();
    }
    fn defer(&mut self, i: u32) -> bool {
        if self.painting == Some(i) {
            return false;
        }
        self.sink.defer(i)
    }
}
