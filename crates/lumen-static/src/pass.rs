//! Frame entry points.
//!
//! Thin wrappers over the recursive `Widget` methods, kept as named
//! functions so hosts, tests and benchmarks have one obvious way in.
//! The flat iterative passes they replaced are recorded in
//! `docs/results-traversal.md`.

use crate::desc::{Constraints, NodeState, Point};
use crate::display::DisplayList;
use crate::cx::LayoutCx;
use crate::style::Style;
use crate::widget::Widget;
use crate::widget::Node;

/// Lay out and position a whole tree.
/// Lay out with a caller-supplied context (the shaping stack, once one
/// is composed in).
pub fn layout_with<W: Node + ?Sized>(
    tree: &mut W,
    cx: &mut LayoutCx,
    style: &[Style],
    st: &mut [NodeState],
    limits: Constraints,
) {
    tree.layout(cx, limits, style, st, 0);
    tree.place(Point::default(), style, st, 0);
}

/// Lay out with the built-in metrics — what tests and simple apps use
/// until a real text stack is wired in.
pub fn layout<W: Node + ?Sized>(
    tree: &mut W,
    style: &[Style],
    st: &mut [NodeState],
    limits: Constraints,
) {
    let mut m = crate::cx::BlockMetrics;
    let mut cx = LayoutCx::new(&mut m);
    layout_with(tree, &mut cx, style, st, limits);
}


/// Default styles for a tree, with no stylesheet applied — the common
/// case in tests and simple apps.
pub fn default_styles<W: Widget>(tree: &W) -> Vec<Style> {
    (0..W::NODE_COUNT).map(|i| tree.node(i).defaults()).collect()
}

/// Measure only, without positioning — used by tools that need a node's
/// size under hypothetical limits.
pub fn measure<W: Node + ?Sized>(
    tree: &mut W,
    cx: &mut LayoutCx,
    style: &[Style],
    st: &mut [NodeState],
    limits: Constraints,
) -> crate::desc::Size {
    tree.layout(cx, limits, style, st, 0)
}

pub fn paint<W: Node + ?Sized>(tree: &W, st: &[NodeState], out: &mut DisplayList) {
    tree.paint(st, 0, out);
}

pub fn hit_test<W: Node + ?Sized>(
    tree: &W,
    st: &[NodeState],
    x: f32,
    y: f32,
) -> Option<crate::hit::Hit> {
    tree.hit(st, 0, x, y)
}

/// Dispatch one input event — plan E2, on the recursive path.
///
/// Pointer events hit-test; keyboard events go to `focus`. The event is
/// offered to the hit node and then to each ancestor until one consumes
/// it, so a container can act as a fallback for content that ignored it.
pub fn dispatch<S: crate::event::App>(
    state: &mut S,
    style: &[Style],
    st: &[NodeState],
    focus: &mut Option<u32>,
    ev: &crate::event::Event,
) -> bool {
    use crate::event::{Event, EventCtx, Handlers, NodeSel};
    use crate::widget::Widget;

    let table = <S::Tree as Widget>::SUB_TABLE;
    // `style` is unused by routing today; it is threaded now so hidden
    // subtrees (plan M4/4a) can be skipped without another signature
    // change, and so hosts pass one consistent set of arrays.
    let _ = style;

    let (start, mut ctx, inner) = match ev.position() {
        Some(p) => {
            let Some(h) = state.tree().hit(st, 0, p.x, p.y) else { return false };
            if matches!(ev, Event::PointerDown { .. }) {
                // Focus moves to the deepest focusable node on the hit
                // chain, or clears — what clicking the background does.
                let mut f = None;
                let mut i = h.node as usize;
                loop {
                    if table[i].is_focusable() {
                        f = Some(i as u32);
                        break;
                    }
                    if table[i].parent == crate::desc::NO_PARENT {
                        break;
                    }
                    i = table[i].parent as usize;
                }
                *focus = f;
            }
            let ctx = EventCtx {
                node: h.node,
                row: h.inner.map(|x| x.row),
                inner: h.inner.map_or(0, |x| x.node),
                pos: p,
                local: h.local,
            };
            (h.node as usize, ctx, h.inner.map(|x| (x.row, x.node)))
        }
        None => match *focus {
            // Bounds-checked where the index *enters*, not where it is
            // used: `Host.focus` is public, and a hot reload can shrink
            // the tree under a focus set against the old one. `table[i]`
            // below would then panic on a safe API.
            //
            // This is currently unreachable, and only because the
            // `PointerUp` gate under this match drops every keyboard
            // event before the loop runs. That gate is M9's to remove;
            // whoever removes it must not also have to rediscover this.
            // `docs/plan-review-fixes.md` F1.8.
            Some(f) if (f as usize) < table.len() => (f as usize, EventCtx::at(f), None),
            Some(_) => {
                *focus = None;
                return false;
            }
            None => return false,
        },
    };

    // Handlers are *click* handlers: activation is pointer-up, matching
    // the widget-level policy the old per-kind event op encoded. Focus
    // still moves on pointer-down above. Richer routing (key activation,
    // hover, drag) extends `Handlers` when those widgets exist.
    if !matches!(ev, Event::PointerUp { .. }) {
        return false;
    }

    let mut i = start;
    loop {
        ctx.node = i as u32;
        if i != start {
            // Only the node actually hit owns the row detail.
            ctx.row = None;
            ctx.inner = 0;
        }
        let sel = NodeSel { i, inner: if i == start { inner } else { None } };
        // Copy the handler out; the shared borrow of the tree ends here,
        // before `&mut S` exists.
        let handler = state.tree().handler(sel);
        if let Some(h) = handler {
            h(state, &ctx);
            return true;
        }
        if table[i].parent == crate::desc::NO_PARENT {
            return false;
        }
        i = table[i].parent as usize;
    }
}
