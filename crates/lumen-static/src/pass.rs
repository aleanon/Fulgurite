//! Frame entry points.
//!
//! Thin wrappers over the recursive `Widget` methods, kept as named
//! functions so hosts, tests and benchmarks have one obvious way in.
//! The flat iterative passes they replaced are recorded in
//! `docs/results-traversal.md`.

use crate::desc::{Constraints, NodeState, Point};
use crate::display::Painter;
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

/// Paint the whole tree into `out`.
///
/// `out` is a [`crate::Painter`], so this is equally the entry point
/// for a host that collects a [`crate::DisplayList`] and for one that
/// rasterises immediately and keeps nothing.
pub fn paint<W: Node + ?Sized>(tree: &W, st: &[NodeState], out: &mut dyn Painter) {
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
    st: &mut [NodeState],
    focus: &mut Option<u32>,
    ev: &crate::event::Event,
) -> bool {
    use crate::event::{Event, EventCtx, Handlers, Key, NodeSel};
    use crate::widget::Widget;

    let table = <S::Tree as Widget>::SUB_TABLE;

    // C5: focus that is no longer on screen does not keep receiving
    // keystrokes.
    //
    // The scenario: focus a control in `Either::A`, switch to `B`, keep
    // typing. The hidden branch is still in the table and still
    // focusable by index, so without this it takes every keystroke —
    // invisibly. Checked here, once, before anything is routed, rather
    // than at each use: `Host.focus` is public and a stale value can
    // arrive from anywhere.
    //
    // Both tests are needed. `visible` catches a styled-away node;
    // *area* catches the ones hidden by `layout` zeroing their subtree,
    // which is how `Option`, `Either` and a hidden subtree all express
    // absence — and none of those touches `Style::visible`.
    if let Some(f) = *focus {
        let i = f as usize;
        let gone = i >= table.len()
            || !style[i].visible
            || st[i].size.w <= 0.0
            || st[i].size.h <= 0.0;
        if gone {
            *focus = None;
        }
    }

    // Publish focus into the state array, so a widget's `paint` can see
    // it (a caret is drawn only in the focused field). Every node, so a
    // focus that moved does not leave the old one lit.
    for (i, n) in st.iter_mut().enumerate() {
        n.focused = *focus == Some(i as u32);
    }

    // C5: Tab moves focus in declaration order, which for a pre-order
    // table is simply the next focusable index, wrapping. Consumed here
    // — it never reaches a handler, the way it never does on any
    // platform.
    //
    // Shift+Tab is not implemented because `Event::Key` carries no
    // modifier state; adding one is a change to the event vocabulary and
    // to every shell that fills it in.
    if matches!(ev, crate::event::Event::Key { key: crate::event::Key::Tab, down: true }) {
        *focus = next_focusable(table, st, style, *focus);
        return true;
    }

    let (start, mut ctx, inner) = match ev.position() {
        Some(p) => {
            let hit = state.tree().hit(st, 0, p.x, p.y);

            // C6: pointer state, before anything can return early.
            // Hover follows the pointer; press latches on down and
            // releases on up *wherever* the pointer then is — a button
            // must un-press when you drag off it and let go, which is
            // why the release clears every node rather than the hit one.
            match ev {
                Event::PointerMove { .. } => set_hovered(st, hit.map(|h| h.node)),
                Event::PointerDown { .. } => {
                    set_hovered(st, hit.map(|h| h.node));
                    if let Some(h) = hit {
                        st[h.node as usize].pressed = true;
                    }
                }
                Event::PointerUp { .. } => {
                    for n in st.iter_mut() {
                        n.pressed = false;
                    }
                }
                _ => {}
            }

            let Some(h) = hit else { return false };
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
                event: *ev,
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
            // `docs/plan-review-fixes.md` F1.8.
            Some(f) if (f as usize) < table.len() => {
                (f as usize, EventCtx::for_event(f, *ev), None)
            }
            Some(_) => {
                *focus = None;
                return false;
            }
            // C1: a keystroke with nothing focused is not an error, it
            // is a keystroke nobody wanted. Reported as unhandled so a
            // shell can use it for an accelerator.
            None => return false,
        },
    };

    // `PointerMove` is deliberately not routed: it fires continuously,
    // and its only consumer is the hover state set above, which needs no
    // handler. Routing it would walk the ancestor chain on every mouse
    // motion looking for something that is almost never there.
    if matches!(ev, Event::PointerMove { .. }) {
        return false;
    }

    // Pass 1 — C3: the widget itself, deepest first.
    //
    // Until 2026-09-04 `dispatch` returned here for everything but
    // `PointerUp`, so `input.type` and `input.scroll` were enqueued,
    // drained and dropped: finished-but-unreachable, the failure mode
    // `crates/lumen-static-agent/src/audit.rs` exists to catch. A
    // scroller consumes a wheel here; a text field will consume a
    // keystroke. The `bool` is also C4's scroll chaining — a scroller at
    // its end declines and its parent gets the event.
    // The row detail belongs to the node actually hit, and *both* walks
    // start there — so pass 1 must not leave the ctx it cleared for the
    // ancestors behind for pass 2. Saved and restored rather than
    // rebuilt: a fresh ctx would have to re-derive `local`, which only
    // the hit knows.
    let (row0, inner0) = (ctx.row, ctx.inner);
    {
        let mut i = start;
        loop {
            ctx.node = i as u32;
            if i != start {
                ctx.row = None;
                ctx.inner = 0;
            }
            if state.tree_mut().node_mut(i).on_event(ev, &ctx) {
                return true;
            }
            if table[i].parent == crate::desc::NO_PARENT {
                break;
            }
            i = table[i].parent as usize;
        }
    }
    ctx.row = row0;
    ctx.inner = inner0;

    // Pass 2 — activation: the application's handler.
    //
    // These stay *click* handlers, which is what every one written so
    // far assumes: `|c, _| c.count += 1` ignores the ctx entirely, so
    // routing arbitrary keys to it would make every keystroke press
    // every button. Enter on the focused node activates too — keyboard
    // activation, which is what makes a `Button` reachable without a
    // pointer at all.
    //
    // **Space does not**, and should: every desktop toolkit toggles a
    // checkbox with it. The `Key` enum in `event.rs` has no variant for
    // it to match on, so this is an event-vocabulary gap rather than an
    // omission here — planned as M11/T11.1 (`docs/plan-widgets.md`). Until
    // 2026-09-04 this comment claimed Space already worked, which no
    // static check could catch: the checks verify that backticked
    // *identifiers* exist, not that prose is true.
    let activates = matches!(
        ev,
        Event::PointerUp { .. }
            | Event::Key { key: Key::Enter, down: true }
            | Event::Key { key: Key::Space, down: true }
    );
    if !activates {
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

/// The focusable node after `from` in declaration order, wrapping.
///
/// Skips anything with no area or `visible: false` — a Tab stop you
/// cannot see is a Tab stop that loses the user.
fn next_focusable(
    table: &[crate::desc::NodeDesc],
    st: &[NodeState],
    style: &[Style],
    from: Option<u32>,
) -> Option<u32> {
    let n = table.len();
    if n == 0 {
        return None;
    }
    let start = from.map_or(0, |f| f as usize + 1);
    (0..n)
        .map(|k| (start + k) % n)
        .find(|&i| {
            table[i].is_focusable()
                && style[i].visible
                && st[i].size.w > 0.0
                && st[i].size.h > 0.0
        })
        .map(|i| i as u32)
}

/// Mark `node` hovered and nothing else.
///
/// Every node, not just the previously-hovered one: a frame may have
/// changed the tree under us, and a stale bit is a control that stays
/// lit after the pointer has left it.
fn set_hovered(st: &mut [NodeState], node: Option<u32>) {
    for n in st.iter_mut() {
        n.hovered = false;
    }
    if let Some(i) = node {
        st[i as usize].hovered = true;
    }
}
