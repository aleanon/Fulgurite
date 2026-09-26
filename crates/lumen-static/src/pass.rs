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
    dispatch_overlaid_into(state, style, st, input, ov, ev, &mut Consumed::default())
}

/// **The nodes an event changed by being consumed** — M29/T29.6
/// (`docs/plan-damage-precision.md`).
///
/// A handler writes only the state of a split application, and a binding
/// then says where the tree changed. But a widget that *consumes* an
/// event writes itself — a `TextInput` takes a character, a `Slider`
/// moves, a popup closes — and no binding reports that. This is the set
/// of those nodes: possibly several (a press outside dismisses every
/// open overlay), possibly none (a focus move, which T25.0's marks
/// cover). Fixed capacity; past it the set **overflows**, and the host
/// answers with everything rather than drop a node.
#[derive(Clone, Copy, Debug, Default)]
pub struct Consumed {
    nodes: [u32; Consumed::CAP],
    len: usize,
    overflow: bool,
}

impl Consumed {
    const CAP: usize = 8;

    pub fn push(&mut self, i: usize) {
        if self.nodes[..self.len].contains(&(i as u32)) {
            return;
        }
        if self.len == Self::CAP {
            self.overflow = true;
        } else {
            self.nodes[self.len] = i as u32;
            self.len += 1;
        }
    }

    pub fn nodes(&self) -> &[u32] {
        &self.nodes[..self.len]
    }

    /// Whether a node was dropped for want of room: mark everything.
    pub fn overflowed(&self) -> bool {
        self.overflow
    }
}

/// [`dispatch_overlaid`], recording what it consumed on into `consumed`.
pub fn dispatch_overlaid_into<S: crate::event::App, O: crate::overlay::OverlaySink>(
    state: &mut S,
    style: &[Style],
    st: &mut [NodeState],
    input: &mut crate::event::InputState,
    ov: &O,
    ev: &crate::event::Event,
    consumed: &mut Consumed,
) -> bool {
    let handled = overlaid(state, style, st, input, ov, ev, consumed);
    publish_focus(st, input);
    handled
}

/// [`dispatch_overlaid_into`]'s body, before focus is published.
fn overlaid<S: crate::event::App, O: crate::overlay::OverlaySink>(
    state: &mut S,
    style: &[Style],
    st: &mut [NodeState],
    input: &mut crate::event::InputState,
    ov: &O,
    ev: &crate::event::Event,
    consumed: &mut Consumed,
) -> bool {
    use crate::event::{Event, Key};

    // **The release of a press that dismissed an overlay is consumed
    // too** — M33/T33.3. The press was eaten below; without this the
    // release activated whatever it landed on, so the click that closed
    // a menu also pressed the button under it. The release still ends
    // the press everywhere, as any release does.
    match ev {
        Event::PointerDown { .. } => {
            input.swallow_release = false;
            // A press latches below only if it reaches ordinary
            // dispatch; one that dismissed an overlay latches nothing.
            input.press = None;
        }
        Event::PointerUp { .. } | Event::PointerCancel { .. } if input.swallow_release => {
            input.swallow_release = false;
            input.press = None;
            release_press(st, &mut input.pressed, &mut input.state_writes);
            return true;
        }
        _ => {}
    }

    // A cancel is not a press or a release anywhere: it goes to the
    // capture, never to what an overlay has under the point (T44.4).
    if matches!(ev, Event::PointerCancel { .. }) {
        return dispatch_at(state, style, st, input, ev, None, consumed);
    }

    // Escape closes the topmost overlay and stops there.
    if matches!(ev, Event::Key { key: Key::Escape, down: true }) && !ov.is_empty() {
        let top = ov.at(ov.len() - 1) as usize;
        if state.tree_mut().node_mut(top).dismiss() {
            input.focus = None;
            consumed.push(top);
            tell_owner(state, top, ev, input);
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
                // A passive overlay (a tooltip) closes without taking the
                // press: if every overlay this closed was passive, the
                // press goes on to ordinary dispatch below.
                let mut consumes_press = false;
                for k in (0..ov.len()).rev() {
                    let i = ov.at(k) as usize;
                    let consumes = state.tree().node(i).dismissal_consumes_press();
                    if state.tree_mut().node_mut(i).dismiss() {
                        consumes_press |= consumes;
                        consumed.push(i);
                        tell_owner(state, i, ev, input);
                    }
                }
                if consumes_press {
                    input.focus = None;
                    input.swallow_release = true;
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
                        consumed.push(n);
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

    dispatch_at(state, style, st, input, ev, on_overlay, consumed)
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
    let handled = dispatch_at(state, style, st, input, ev, None, &mut Consumed::default());
    publish_focus(st, input);
    handled
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
    consumed: &mut Consumed,
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
    // **A cancel ends the gesture and activates nothing** — M44/T44.4.
    // The press is cleared as a release clears it, the capture holder is
    // told (it may keep drag state of its own), and the capture ends. No
    // hit test and no activation: a cancelled tap is not a click.
    if let Event::PointerCancel { x, y } = *ev {
        release_press(st, &mut input.pressed, &mut input.state_writes);
        input.press = None;
        let Some(cap) = input.capture.take() else { return false };
        let cap = cap as usize;
        if cap >= table.len() {
            return false;
        }
        let s = &st[cap];
        let p = Point { x, y };
        let cx = EventCtx {
            event: *ev,
            node: cap as u32,
            row: None,
            inner: 0,
            pos: p,
            local: Point { x: p.x - s.pos.x, y: p.y - s.pos.y },
            button: ev.button(),
            clicks: input.clicks,
            mods: input.mods,
        };
        let taken = state.tree_mut().node_mut(cap).on_event(ev, &cx);
        if taken {
            consumed.push(cap);
        }
        return taken;
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
    //
    // **The same test ends a capture** (M44/T44.3, R3-F4): see the
    // capture block below.
    if let Some(f) = *focus {
        if gone(table.len(), style, st, f as usize) {
            *focus = None;
        }
    }

    // Focus is published into the state array by the public entry
    // points, **after** the event has moved it — `publish_focus`.

    // C5: Tab moves focus in declaration order, which for a pre-order
    // table is simply the next focusable index, wrapping. Consumed here
    // — it never reaches a handler, the way it never does on any
    // platform.
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
    // a toolbar or a menu is, and it needs no new field to express. It
    // takes effect only when the parent has another focusable child.
    //
    // **A widget that uses arrows owns them** — M44/T44.1
    // (`docs/plan-input-semantics.md` §2.1). Until then this ran before
    // the widget and never asked, so a text field beside a button could
    // not move its caret: the arrow moved focus instead (R3-F1, R10-F3).
    // Now a focused node answering [`Node::owns_arrows`] skips the rule
    // entirely and the key reaches Pass 1 below, which records it in
    // `Consumed` like any other key the widget takes — the fix adds no
    // call site that could forget to (§0 rule 2). Whether the widget then
    // consumes the key does not matter: an arrow at a field's edge stays
    // in the field, as on every desktop platform.
    //
    // **The group is walked, not collected** (R3-F9). It used to be
    // gathered into a sixteen-slot buffer, so a member past the
    // sixteenth was unreachable and an arrow on one was a dead key. The
    // walk scans the parent's subtree from the focused index, in the
    // arrow's direction, wrapping, for the next focusable child of the
    // same parent: O(subtree), no allocation, no cap.
    //
    // [`Node::owns_arrows`]: crate::widget::Node::owns_arrows
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
                if i < table.len()
                    && table[i].parent != crate::desc::NO_PARENT
                    && !state.tree().node(i).owns_arrows()
                {
                    let p = table[i].parent as usize;
                    if let Some(j) = next_in_group(table, st, style, p, i, dir > 0) {
                        *focus = Some(j as u32);
                        return true;
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
    //
    // **A capture ends when its node leaves the screen** — M44/T44.3,
    // R3-F4, C5's rule applied to the pointer. A slider hidden mid-drag
    // (an `Either` switched away, a styled-away subtree) used to keep
    // every move until the release, changing a value nobody could see.
    // So a captured node that is gone loses the capture and the press
    // with it — pressed bits, the press shade, the press latch — and the
    // event falls through to ordinary dispatch rather than reaching it.
    if let Some(cap) = *capture {
        let cap = cap as usize;
        if cap < table.len() && gone(table.len(), style, st, cap) {
            *capture = None;
            release_press(st, &mut input.pressed, &mut input.state_writes);
            input.press = None;
        } else if cap >= table.len() {
            *capture = None;
        } else if let Some(p) = ev.position() {
            if matches!(ev, Event::PointerMove { .. } | Event::PointerUp { .. }) {
                if matches!(ev, Event::PointerUp { .. }) {
                    release_press(st, &mut input.pressed, &mut input.state_writes);
                    // The gesture the capture held ends here, and a
                    // captured release activates nothing (T44.2).
                    input.press = None;
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
                if taken {
                    consumed.push(cap);
                }
                if matches!(ev, Event::PointerUp { .. }) {
                    *capture = None;
                }
                return taken;
            }
        }
    }

    // The press a release completes, taken from `input` as the release
    // arrives (T44.2): `Some(None)` is a release with no press latched.
    let mut released: Option<Option<crate::event::PressLatch>> = None;
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
            // why the release clears the *pressed* node rather than the
            // hit one. The indices go into `input` beside the bits, so the
            // host can see that they changed (M25/T25.0) — and so that a
            // change writes the two nodes it concerns and no others
            // (M44/T44.6): these were sweeps of the whole state array,
            // twice per move.
            match ev {
                Event::PointerMove { .. } => {
                    let was = input.hovered;
                    input.hovered = hit.map(|h| h.node);
                    input.state_writes.add(set_hovered(st, was, input.hovered));
                }
                Event::PointerDown { .. } => {
                    let was = input.hovered;
                    input.hovered = hit.map(|h| h.node);
                    input.state_writes.add(set_hovered(st, was, input.hovered));
                    // A press with one still held (a second pointer)
                    // lets the first go, so no bit is left behind.
                    release_press(st, &mut input.pressed, &mut input.state_writes);
                    if let Some(h) = hit {
                        st[h.node as usize].pressed = true;
                        input.state_writes.add(1);
                        input.pressed = Some(h.node);
                    }
                    // The press's identity, for the release to compare
                    // against (T44.2).
                    input.press = hit.map(|h| crate::event::PressLatch {
                        node: h.node,
                        row: h.inner.map(|x| x.row),
                        generation: state.tree().node(h.node as usize).row_generation(),
                    });
                }
                Event::PointerUp { .. } => {
                    release_press(st, &mut input.pressed, &mut input.state_writes);
                    released = Some(input.press.take());
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
                consumed.push(i);
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
    // every button. Enter and Space on the focused node activate too —
    // keyboard activation, which is what makes a `Button` reachable
    // without a pointer at all, and Space is what every desktop toolkit
    // toggles a checkbox with. (Space arrived with `Key::Space` in
    // M11/T11.1; until M44/T44.5 this comment still said it did not
    // activate, directly above the line that makes it — the checks verify
    // that backticked identifiers exist, not that prose is true.)
    //
    // **A pointer click is a press and a release on the same item** —
    // M44/T44.2 (`docs/plan-input-semantics.md` §2.2). A handler runs on
    // the release only if its node also contains the press, so a drag
    // from one button to another activates neither — the release is
    // offered up the chain, as on the web, and fires on the nearest node
    // holding both ends that has a handler. A row's handler also needs
    // the same row, under the same `row_generation`: an insertion between
    // press and release puts a different item under a pointer that never
    // moved, and that click is cancelled rather than delivered to it.
    let activates = matches!(
        ev,
        Event::PointerUp { .. }
            | Event::Key { key: Key::Enter, down: true }
            | Event::Key { key: Key::Space, down: true }
    );
    if !activates {
        return false;
    }

    // What the release is over, for comparing with the press.
    let now = released.map(|press| (press, state.tree().node(start).row_generation()));
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
        let same_item = match now {
            None => true, // a key, not a release
            Some((press, generation)) => {
                press_is_within(table, press, i, sel.inner.map(|x| x.0), start, generation)
            }
        };
        if let (Some(h), true) = (handler, same_item) {
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

/// Whether a release's candidate handler at `i` completes the latched
/// press — M44/T44.2.
///
/// `row` is the release's row when `i` is the hit node and the hit was on
/// a row; then the press must be on that node, that row, under the same
/// `generation`. Otherwise `i` must contain the press's node: `i` is the
/// press's node or one of its ancestors. A release with nothing latched
/// completes nothing.
fn press_is_within(
    table: &[crate::desc::NodeDesc],
    press: Option<crate::event::PressLatch>,
    i: usize,
    row: Option<u64>,
    start: usize,
    generation: u64,
) -> bool {
    let Some(p) = press else { return false };
    if let Some(row) = row {
        return i == start
            && p.node as usize == start
            && p.row == Some(row)
            && p.generation == generation;
    }
    let mut j = p.node as usize;
    loop {
        if j == i {
            return true;
        }
        if j >= table.len() || table[j].parent == crate::desc::NO_PARENT {
            return false;
        }
        j = table[j].parent as usize;
    }
}

/// **Tell a dismissed overlay's owner** — M33/T33.3.
///
/// The overlay the pass just closed is a node the owner *contains* — a
/// `Select`'s popup — so the question goes to its parent, and only one
/// level: a derived container answers `None` at its own index, and asking
/// further up could reach an unrelated handler. Asked with
/// [`NodeSel::dismissed`](crate::event::NodeSel::dismissed), which no
/// activation ever sends, so an owner's dismissal handler cannot be run
/// by a click.
fn tell_owner<S: crate::event::App>(
    state: &mut S,
    overlay: usize,
    ev: &crate::event::Event,
    input: &crate::event::InputState,
) {
    let table = <S::Tree as crate::widget::Widget>::SUB_TABLE;
    let owner = table[overlay].parent;
    if owner == crate::desc::NO_PARENT {
        return;
    }
    let handler = crate::event::Handlers::handler(state.tree(), crate::event::NodeSel::dismissed(owner as usize));
    if let Some(h) = handler {
        let ctx = crate::event::EventCtx {
            event: *ev,
            node: owner,
            row: None,
            inner: 0,
            pos: ev.position().unwrap_or(Point { x: 0.0, y: 0.0 }),
            local: Point { x: 0.0, y: 0.0 },
            button: ev.button(),
            clicks: input.clicks,
            mods: input.mods,
        };
        h(state.state_mut(), &ctx);
    }
}

/// C5's test: whether node `i` is **gone** from the screen — outside the
/// table, styled invisible, or laid out with no area.
///
/// Both of the last two are needed: `visible` catches a styled-away
/// node, and area catches one hidden by `layout` zeroing its subtree,
/// which is how `Option`, `Either` and a hidden subtree express absence.
/// One function, so focus (C5) and capture (M44/T44.3) cannot disagree
/// about what "gone" means.
fn gone(len: usize, style: &[Style], st: &[NodeState], i: usize) -> bool {
    i >= len || !style[i].visible || st[i].size.w <= 0.0 || st[i].size.h <= 0.0
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

/// The next member of `i`'s focus group after `i` — forwards or
/// backwards, wrapping — or `None` when `i` is not a member or is the
/// only one (M44/T44.1).
///
/// A member is a focusable **immediate child** of `parent`, not any
/// focusable in its subtree — that distinction is the difference
/// between arrows navigating a radio group and arrows jumping out of a
/// nested control. The scan covers the parent's subtree range, which
/// holds every child; it is linear in that range and has no cap.
fn next_in_group(
    table: &[crate::desc::NodeDesc],
    st: &[NodeState],
    style: &[Style],
    parent: usize,
    i: usize,
    forward: bool,
) -> Option<usize> {
    let range = crate::style::subtree_range(table, parent);
    let (lo, n) = (range.start, range.end.min(table.len()).saturating_sub(range.start));
    let member = |j: usize| table[j].parent as usize == parent && focusable(table, st, style, j);
    if n == 0 || i < lo || i >= lo + n || !member(i) {
        return None;
    }
    let at = i - lo;
    (1..n)
        .map(|k| lo + if forward { (at + k) % n } else { (at + n - k) % n })
        .find(|&j| member(j))
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

/// Move the hover bit from `was` to `now`, writing those two nodes and
/// no others — M44/T44.6.
///
/// Returns how many state writes it made, for
/// [`InputState::state_writes`](crate::event::InputState::state_writes).
/// Each index is bounds-checked. A sweep of every node used to run here
/// on every move and press, "because a frame may have changed the tree
/// under us"; but a state array is only ever replaced whole (a new
/// `Host`, whose bits all start clear) or swept by panic recovery, so the
/// recorded index is enough.
fn set_hovered(st: &mut [NodeState], was: Option<u32>, now: Option<u32>) -> usize {
    let mut writes = 0;
    if let Some(i) = was.filter(|&i| Some(i) != now && (i as usize) < st.len()) {
        st[i as usize].hovered = false;
        writes += 1;
    }
    if let Some(i) = now.filter(|&i| (i as usize) < st.len()) {
        if !st[i as usize].hovered {
            st[i as usize].hovered = true;
            writes += 1;
        }
    }
    writes
}

/// End the press shade: clear the pressed node's bit and forget it —
/// one write, not a sweep (M44/T44.6).
fn release_press(st: &mut [NodeState], pressed: &mut Option<u32>, writes: &mut crate::event::WriteCount) {
    if let Some(i) = pressed.take().filter(|&i| (i as usize) < st.len()) {
        st[i as usize].pressed = false;
        writes.add(1);
    }
}

/// Publish `input.focus` into the state array, so a widget's `paint`
/// can see it (a caret is drawn only in the focused field) — M44/T44.6.
///
/// Clears the node focus was last published on and sets the new one: at
/// most two writes, where this was a write to every node on every event.
/// And it runs **after** the event, from the public entry points: it used
/// to run at the top of `dispatch_at`, before Tab or an arrow moved focus,
/// so the frame after a focus move drew the ring on the node focus had
/// just left, until the next event republished it. The new node's bit is
/// checked rather than assumed, so a state array replaced under a kept
/// focus gets its bit back.
fn publish_focus(st: &mut [NodeState], input: &mut crate::event::InputState) {
    let now = input.focus.filter(|&f| (f as usize) < st.len());
    if let Some(old) = input.focus_published.filter(|&o| Some(o) != now && (o as usize) < st.len()) {
        st[old as usize].focused = false;
        input.state_writes.add(1);
    }
    if let Some(f) = now {
        if !st[f as usize].focused {
            st[f as usize].focused = true;
            input.state_writes.add(1);
        }
    }
    input.focus_published = now;
}
