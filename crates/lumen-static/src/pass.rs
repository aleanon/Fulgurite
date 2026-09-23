//! Frame entry points.
//!
//! Thin wrappers over the recursive `Widget` methods, kept as named
//! functions so hosts, tests and benchmarks have one obvious way in.
//! The flat iterative passes they replaced are recorded in
//! `docs/results-traversal.md`.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1).
#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

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
///
/// The allocating convenience over [`default_styles_into`]. An app that
/// owns its own style array wants that one; this is what almost every
/// caller in this repository wants.
#[cfg(feature = "alloc")]
pub fn default_styles<W: Widget>(tree: &W) -> Vec<Style> {
    let mut out = vec![Style::default(); W::NODE_COUNT];
    default_styles_into(tree, &mut out);
    out
}

/// Write a tree's default styles into caller-owned storage.
///
/// The alloc-free half of [`default_styles`], and the reason M13's
/// static path is reachable at all: `default_styles` returning a `Vec`
/// was the only way to build a style array, so an app that owned its
/// own `[Style; N]` still had to allocate one to fill it
/// (`docs/plan-embedded.md` T13.3).
///
/// `out` must hold at least `W::NODE_COUNT` styles. It may be longer —
/// the trailing entries are left alone, so a fixed-capacity array
/// shared between trees works.
///
/// # Panics
///
/// If `out` is shorter than `W::NODE_COUNT`. The check is explicit
/// rather than left to indexing so the message names both numbers: the
/// caller's array is a `const` they can fix, and "index out of bounds"
/// would not say what to fix it to.
pub fn default_styles_into<W: Widget>(tree: &W, out: &mut [Style]) {
    assert!(
        out.len() >= W::NODE_COUNT,
        "style array holds {} but the tree has {} nodes",
        out.len(),
        W::NODE_COUNT
    );
    // **From the seed table, not the live tree** — the same correction
    // the cascade got in M24/T24.1, in the second function that had the
    // problem. `tree.node(i)` answers with the *container* for a hidden
    // subtree (an `Option::None`, an inactive enum variant), so those
    // nodes were seeded with the wrong type's defaults here exactly as
    // they were there. Found by `tests/hatches.rs`, which read an
    // inactive variant's width and got the enum's.
    let _ = tree;
    for (i, slot) in out.iter_mut().enumerate().take(W::NODE_COUNT) {
        *slot = (W::SEEDS[i].defaults)();
    }
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

/// Paint, then paint the overlays — `docs/plan-capabilities.md` T15.0.
///
/// Phase 1 is the ordinary pre-order walk, with a [`Deferring`] painter
/// wrapped around `out` so a widget may ask for a subtree to be painted
/// later instead of here. Phase 2 repaints each deferred subtree
/// **outside every clip scope**, in the order it was deferred, so later
/// overlays land on top of earlier ones and no z-index is needed.
///
/// Deferrals made *during* phase 2 are painted in the next round — a
/// submenu inside a menu — up to [`crate::overlay::MAX_ROUNDS`].
///
/// With [`NoOverlays`](crate::NoOverlays) every deferral is refused, the
/// loop never runs, and this is `paint` with one `is_empty` check.
///
/// [`Deferring`]: crate::overlay::Deferring
pub fn paint_overlaid<W: Node + ?Sized, O: crate::overlay::OverlaySink>(
    tree: &W,
    st: &[NodeState],
    out: &mut dyn Painter,
    ov: &mut O,
) {
    use crate::overlay::{Deferring, MAX_ROUNDS};
    ov.clear();
    {
        let mut d = Deferring::new(out, ov);
        tree.paint(st, 0, &mut d);
    }

    let mut done = 0;
    for _ in 0..MAX_ROUNDS {
        let upto = ov.len();
        if done == upto {
            break;
        }
        for k in done..upto {
            // Copied out before the sink is borrowed again.
            let i = ov.at(k) as usize;
            let mut d = Deferring::repainting(out, ov, i as u32);
            // Painted at its own index, the way the agent's journal
            // repaints a single node. Nothing above it in the tree runs,
            // so no ancestor's clip applies — which is the point.
            tree.node(i).paint(st, i, &mut d);
        }
        done = upto;
    }
}

/// Dispatch with overlays on top — `docs/plan-capabilities.md` T15.1.
///
/// Two things ordinary [`dispatch`] cannot do, both of which every
/// overlay needs:
///
/// - **Route to what is on top.** Pointer events hit-test through
///   [`hit_test_overlaid`], so a click on a dropdown reaches the
///   dropdown and not the row underneath it.
/// - **Answer a click that hits nothing.** A press outside every open
///   overlay dismisses them. That event reaches no widget by
///   definition, so no widget can react to it — the pass has to, which
///   is why `plan-widgets.md` T11.5 listed dismissal as a blocker
///   rather than a detail.
///
/// Escape dismisses the topmost overlay before anything else sees it,
/// which is what stops a text field inside a modal from eating it.
///
/// Returns whether anything handled the event.
pub fn dispatch_overlaid<S: crate::event::App, O: crate::overlay::OverlaySink>(
    state: &mut S,
    style: &[Style],
    st: &mut [NodeState],
    input: &mut crate::event::InputState,
    ov: &O,
    ev: &crate::event::Event,
) -> bool {
    use crate::event::{Event, Key};

    // Escape closes the topmost overlay and stops there.
    if matches!(ev, Event::Key { key: Key::Escape, down: true }) && !ov.is_empty() {
        let top = ov.at(ov.len() - 1) as usize;
        if state.tree_mut().node_mut(top).dismiss() {
            input.focus = None;
            return true;
        }
    }

    // A press outside every open overlay dismisses them all, topmost
    // first, and is *consumed* — the click that closes a menu should
    // not also press the button underneath it, which is what every
    // desktop toolkit does and what a user expects.
    if matches!(ev, Event::PointerDown { .. }) && !ov.is_empty() {
        if let Some(p) = ev.position() {
            let inside = (0..ov.len()).any(|k| {
                let i = ov.at(k) as usize;
                state.tree().node(i).hit(st, i, p.x, p.y).is_some()
            });
            if !inside {
                let mut closed = false;
                for k in (0..ov.len()).rev() {
                    let i = ov.at(k) as usize;
                    closed |= state.tree_mut().node_mut(i).dismiss();
                }
                if closed {
                    input.focus = None;
                    return true;
                }
            }
        }
    }

    // Tab is trapped inside the topmost overlay. A modal that lets Tab
    // walk out to the window behind it is a modal a keyboard user
    // cannot use, and it is the half of "modal" that is not painting.
    if matches!(ev, Event::Key { key: Key::Tab, down: true }) && !ov.is_empty() {
        let top = ov.at(ov.len() - 1) as usize;
        let table = <S::Tree as crate::widget::Widget>::SUB_TABLE;
        let span = crate::style::subtree_range(table, top);
        input.focus = next_focusable_in(table, st, style, input.focus, span);
        return true;
    }

    // Otherwise ordinary dispatch, with overlays taking pointer hits
    // first. `dispatch` does its own hit test, so an overlay hit is
    // routed by index here rather than by re-walking.
    let mut on_overlay = None;
    if let Some(p) = ev.position() {
        if !ov.is_empty() {
            for k in (0..ov.len()).rev() {
                let i = ov.at(k) as usize;
                if let Some(h) = state.tree().node(i).hit(st, i, p.x, p.y) {
                    let cx = crate::event::EventCtx {
                        event: *ev,
                        node: h.node,
                        row: h.inner.map(|x| x.row),
                        inner: h.inner.map_or(0, |x| x.node),
                        pos: p,
                        local: h.local,
                        button: ev.button(),
                        clicks: input.clicks,
                        mods: input.mods,
                    };
                    let n = h.node as usize;
                    if state.tree_mut().node_mut(n).on_event(ev, &cx) {
                        return true;
                    }
                    // Not consumed by the overlay's own widget: fall
                    // through to activation, **on this hit**. The
                    // ordinary walk cannot find it again — see
                    // `dispatch_at`.
                    on_overlay = Some(h);
                    break;
                }
            }
        }
    }

    dispatch_at(state, style, st, input, ev, on_overlay)
}

/// Hit-test with the overlays from the last [`paint_overlaid`] on top.
///
/// Deferred subtrees are tested **first and in reverse**, because the
/// last one painted is the topmost and a click belongs to what a person
/// can see. Then the ordinary walk.
///
/// The overlay list is the one paint produced, deliberately: what can be
/// clicked is what was drawn, and a list built any other way could
/// disagree with the screen.
pub fn hit_test_overlaid<W: Node + ?Sized, O: crate::overlay::OverlaySink>(
    tree: &W,
    st: &[NodeState],
    ov: &O,
    x: f32,
    y: f32,
) -> Option<crate::hit::Hit> {
    for k in (0..ov.len()).rev() {
        let i = ov.at(k) as usize;
        if let Some(h) = tree.node(i).hit(st, i, x, y) {
            return Some(h);
        }
    }
    tree.hit(st, 0, x, y)
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
    input: &mut crate::event::InputState,
    ev: &crate::event::Event,
) -> bool {
    dispatch_at(state, style, st, input, ev, None)
}

/// [`dispatch`], with the hit supplied rather than computed.
///
/// **Why this exists:** [`dispatch_overlaid`] has already hit-tested
/// *with the overlays on top*, and the ordinary walk cannot reproduce
/// that result — a container rejects a point outside its own rect
/// before it ever reaches an out-of-flow child, which is exactly what an
/// overlay is. So `dispatch_overlaid` used to fall through to
/// `dispatch`, whose plain hit test answered `None`, and **no handler
/// inside an open popup could ever run** (M24/T24.2). Passing the hit
/// down is what makes "fall through to activation on the overlay's
/// node" — which the comment there already claimed — true.
///
/// `None` means "hit-test as usual", and is computed at the same point
/// in the sequence it always was, so a captured pointer still costs no
/// hit test.
fn dispatch_at<S: crate::event::App>(
    state: &mut S,
    style: &[Style],
    st: &mut [NodeState],
    input: &mut crate::event::InputState,
    ev: &crate::event::Event,
    preset: Option<crate::hit::Hit>,
) -> bool {
    use crate::event::{Event, EventCtx, Handlers, Key, NodeSel};
    use crate::widget::Widget;

    let table = <S::Tree as Widget>::SUB_TABLE;

    // Modifier state is carried *in* the queue and tracked here, so a
    // recorded session replays with the modifiers it was recorded with
    // (`docs/plan-desktop.md` T14.1). Handlers never see this event:
    // they read `ctx.mods`.
    if let Event::Modifiers(m) = ev {
        input.mods = *m;
        return true;
    }
    // The press's click count survives to its release, where activation
    // happens (T14.3).
    if let Event::PointerDown { clicks, .. } = ev {
        input.clicks = *clicks;
    }
    // Whether a focus ring should show, decided by *what kind* of input
    // this is rather than by where focus lands (T15.6). Set before the
    // borrows below split `input`, and by event kind because that is
    // exactly what the distinction is: a keystroke that moves focus
    // makes the ring visible, a press hides it.
    match ev {
        Event::Key {
            key: Key::Tab | Key::Up | Key::Down | Key::Left | Key::Right,
            down: true,
        } => input.focus_visible = true,
        Event::PointerDown { .. } => input.focus_visible = false,
        _ => {}
    }
    let (focus, capture) = (&mut input.focus, &mut input.capture);
    let (mods, clicks) = (input.mods, input.clicks);

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
        // Shift+Tab goes back, which is half of what Tab means and was
        // missing until T15.6 — a form you can only walk forwards is a
        // form you cannot correct.
        *focus = if mods.has(crate::event::Mods::SHIFT) {
            prev_focusable(table, st, style, *focus)
        } else {
            next_focusable(table, st, style, *focus)
        };
        return true;
    }

    // Arrows move focus **within the current group** — T15.6.
    //
    // A group is the focused node's parent: that is what a radio group,
    // a toolbar or a menu is, and it needs no new field to express. Only
    // when the parent has more than one focusable child, so an arrow
    // inside a text field still edits text rather than jumping away.
    if let crate::event::Event::Key { key, down: true } = ev {
        use crate::event::Key;
        let dir = match key {
            Key::Down | Key::Right => 1i32,
            Key::Up | Key::Left => -1,
            _ => 0,
        };
        if dir != 0 {
            if let Some(f) = *focus {
                let i = f as usize;
                if i < table.len() && table[i].parent != crate::desc::NO_PARENT {
                    let p = table[i].parent as usize;
                    let group: heapless_vec::Small = focusable_siblings(table, st, style, p)
                        .fold(heapless_vec::Small::new(), |mut v, x| {
                            v.push(x as u32);
                            v
                        });
                    // One focusable child is not a group: the key
                    // belongs to the widget, which is what keeps Left
                    // and Right editing text inside a field.
                    if group.len() > 1 {
                        if let Some(k) = group.iter().position(|&x| x == f) {
                            let n = group.len();
                            let j = if dir > 0 { (k + 1) % n } else { (k + n - 1) % n };
                            *focus = Some(group.at(j));
                            return true;
                        }
                    }
                }
            }
        }
    }

    // Pointer capture — M11/T11.2.
    //
    // **Before the hit test**, deliberately. A drag has to keep reaching
    // the control it started on after the pointer has left it, and once
    // it has, hit testing finds nothing and the pointer arm below
    // returns early — so a capture checked after it would never fire in
    // the one case it exists for. That is not hypothetical: it is what
    // the first version of this did, and the slider stopped following
    // the pointer the moment it left the track.
    //
    // A captured node is addressed **directly**, with no walk, which is
    // also how M9's objection to routing `PointerMove` stays answered:
    // moves cost one call when a gesture is in flight and nothing
    // otherwise.
    if let Some(cap) = *capture {
        let cap = cap as usize;
        if cap >= table.len() {
            *capture = None;
        } else if let Some(p) = ev.position() {
            if matches!(ev, Event::PointerMove { .. } | Event::PointerUp { .. }) {
                if matches!(ev, Event::PointerUp { .. }) {
                    for n in st.iter_mut() {
                        n.pressed = false;
                    }
                    input.pressed = None;
                }
                let s = &st[cap];
                let cx = EventCtx {
                    event: *ev,
                    node: cap as u32,
                    row: None,
                    inner: 0,
                    pos: p,
                    // Relative to the *captured* node, not to whatever
                    // the pointer is over — the whole point.
                    local: Point { x: p.x - s.pos.x, y: p.y - s.pos.y },
                    button: ev.button(),
                    clicks,
                    mods,
                };
                let taken = state.tree_mut().node_mut(cap).on_event(ev, &cx);
                if matches!(ev, Event::PointerUp { .. }) {
                    *capture = None;
                }
                return taken;
            }
        }
    }

    let (start, mut ctx, inner) = match ev.position() {
        Some(p) => {
            let hit = match preset {
                Some(h) => Some(h),
                None => state.tree().hit(st, 0, p.x, p.y),
            };

            // C6: pointer state, before anything can return early.
            // Hover follows the pointer; press latches on down and
            // releases on up *wherever* the pointer then is — a button
            // must un-press when you drag off it and let go, which is
            // why the release clears every node rather than the hit one.
            // The indices go into `input` beside the bits, so the host can
            // see that they changed (M25/T25.0).
            match ev {
                Event::PointerMove { .. } => {
                    input.hovered = hit.map(|h| h.node);
                    set_hovered(st, input.hovered);
                }
                Event::PointerDown { .. } => {
                    input.hovered = hit.map(|h| h.node);
                    set_hovered(st, input.hovered);
                    if let Some(h) = hit {
                        st[h.node as usize].pressed = true;
                        input.pressed = Some(h.node);
                    }
                }
                Event::PointerUp { .. } => {
                    for n in st.iter_mut() {
                        n.pressed = false;
                    }
                    input.pressed = None;
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
                button: ev.button(),
                clicks,
                mods,
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
                (f as usize, EventCtx::for_event_with(f, *ev, mods), None)
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

    // **A move is not routed; a hover is** (M27/T27.11, finding 3).
    //
    // The move's only consumer is the hover state set above — routing it
    // would make a `Slider` follow a pointer nobody pressed, which
    // `tests/slider.rs` pins. But a widget that paints *rows* has to
    // learn which row, and had no way to: capture is the only path a
    // move ever took, and capture follows a press. So the move is
    // re-asked as `Event::Hover` and offered to the node it is over,
    // once, without the ancestor walk — a container has the hover bit in
    // its own `NodeState` and does not need this.
    if let Event::PointerMove { x, y } = *ev {
        let hover = Event::Hover { x, y };
        let mut hctx = ctx;
        hctx.event = hover;
        return state.tree_mut().node_mut(start).on_event(&hover, &hctx);
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
                // A node that consumed a press owns the gesture until
                // the release, wherever the pointer then goes.
                if matches!(ev, Event::PointerDown { .. }) {
                    *capture = Some(i as u32);
                }
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
            // **`state_mut()`, not `state`** — M33. A handler
            // receives `A::State`, which is the application itself
            // unless it named a `#[state]` field. When it did, the tree
            // is not reachable from here at all.
            h(state.state_mut(), &ctx);
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
/// Whether node `i` can take focus right now.
fn focusable(
    table: &[crate::desc::NodeDesc],
    st: &[NodeState],
    style: &[Style],
    i: usize,
) -> bool {
    i < table.len()
        && table[i].is_focusable()
        && style[i].visible
        && st[i].size.w > 0.0
        && st[i].size.h > 0.0
}

/// The focusable **immediate children** of `parent`, in table order.
///
/// Immediate children, not the whole subtree — that distinction is the
/// difference between arrows navigating a radio group and arrows
/// jumping out of a text field. A field's parent is usually the root,
/// whose *subtree* holds every focusable in the tree; its *children*
/// hold only the field.
fn focusable_siblings<'a>(
    table: &'a [crate::desc::NodeDesc],
    st: &'a [NodeState],
    style: &'a [Style],
    parent: usize,
) -> impl Iterator<Item = usize> + 'a {
    crate::style::subtree_range(table, parent)
        .filter(move |&i| table[i].parent as usize == parent && focusable(table, st, style, i))
}

/// The previous focusable, wrapping — Shift+Tab, and Up/Left.
fn prev_focusable(
    table: &[crate::desc::NodeDesc],
    st: &[NodeState],
    style: &[Style],
    from: Option<u32>,
) -> Option<u32> {
    prev_focusable_in(table, st, style, from, 0..table.len())
}

/// The previous focusable within `scope`, wrapping.
fn prev_focusable_in(
    table: &[crate::desc::NodeDesc],
    st: &[NodeState],
    style: &[Style],
    from: Option<u32>,
    scope: core::ops::Range<usize>,
) -> Option<u32> {
    let (lo, hi) = (scope.start, scope.end.min(table.len()));
    let n = hi.saturating_sub(lo);
    if n == 0 {
        return None;
    }
    // Walk backwards from just before the current focus. `+ n` keeps
    // the modulo positive without signed arithmetic.
    let start = match from {
        Some(f) if (f as usize) >= lo && (f as usize) < hi => (f as usize - lo) + n - 1,
        _ => n - 1,
    };
    (0..n)
        .map(|k| lo + (start + n - k) % n)
        .find(|&i| {
            table[i].is_focusable()
                && style[i].visible
                && st[i].size.w > 0.0
                && st[i].size.h > 0.0
        })
        .map(|i| i as u32)
}

fn next_focusable(
    table: &[crate::desc::NodeDesc],
    st: &[NodeState],
    style: &[Style],
    from: Option<u32>,
) -> Option<u32> {
    next_focusable_in(table, st, style, from, 0..table.len())
}

/// The same, restricted to a subtree — `docs/plan-capabilities.md`
/// T15.1's focus trap.
///
/// `scope` is a half-open range of table indices. Tab wraps **within**
/// it, which is what a modal needs: a dialog that lets Tab walk out to
/// the window behind it is a dialog a keyboard user cannot use.
fn next_focusable_in(
    table: &[crate::desc::NodeDesc],
    st: &[NodeState],
    style: &[Style],
    from: Option<u32>,
    scope: core::ops::Range<usize>,
) -> Option<u32> {
    let (lo, hi) = (scope.start, scope.end.min(table.len()));
    let n = hi.saturating_sub(lo);
    if n == 0 {
        return None;
    }
    // Start after the current focus when it is inside the scope, and at
    // the scope's beginning when it is not — which is what makes the
    // first Tab after a modal opens land inside the modal.
    let start = match from {
        Some(f) if (f as usize) >= lo && (f as usize) < hi => f as usize + 1 - lo,
        _ => 0,
    };
    (0..n)
        .map(|k| lo + (start + k) % n)
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

/// A tiny fixed-capacity vector, for the focus group walk.
///
/// The core is `no_std` with no allocator, and collecting a group of
/// siblings needs somewhere to put them. Sixteen is far more than any
/// radio group, toolbar or menu row has; a container with more
/// focusable children than that navigates the first sixteen, which is
/// a better failure than allocating on a keystroke.
mod heapless_vec {
    pub const CAP: usize = 16;

    #[derive(Clone, Copy)]
    pub struct Small {
        buf: [u32; CAP],
        len: usize,
    }

    impl Small {
        pub fn new() -> Self {
            Small { buf: [0; CAP], len: 0 }
        }
        pub fn push(&mut self, v: u32) {
            if self.len < CAP {
                self.buf[self.len] = v;
                self.len += 1;
            }
        }
        pub fn len(&self) -> usize {
            self.len
        }
        pub fn at(&self, k: usize) -> u32 {
            self.buf[k.min(CAP - 1)]
        }
        pub fn iter(&self) -> impl Iterator<Item = &u32> + '_ {
            self.buf[..self.len].iter()
        }
    }
}
