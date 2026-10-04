//! M27/T27.8 — `SplitPane` and `Collapsible`.
//!
//! The other half of the layout group. `Wrap` and `Grid` became derive
//! axes, because a container's children are its *fields* here and a
//! wrapping row is a way of arranging fields rather than a widget that
//! holds them. These two are widgets, because each holds something the
//! author does not write as a field list: a divider between exactly two
//! panes, and a body that hides.
//!
//! # An accordion is a column of `Collapsible`s
//!
//! Not a widget of its own. In a dynamic tree an `Accordion` takes a
//! `Vec<Section>` and can enforce one-open-at-a-time; here a container's
//! children are its fields, so the same thing is a `column` of
//! `Collapsible`s and the one-open rule is three lines in the
//! application. Inventing an `Accordion<A, B, C>` to own that rule would
//! be a type per section count.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::text::TextStorage;
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

/// Upper bound on a `SplitPane`'s or `Collapsible`'s node count.
pub const MAX_SPLIT_NODES: usize = 256;

// ---- SplitPane -------------------------------------------------------

/// Two panes with a draggable divider between them — side by side, or
/// stacked when `VERTICAL` (M57/B46).
///
/// **The divider stays under the pointer** — M57/B46. The bar's edge was
/// placed at `split · (w − bar)` and a drag set `split = x / w`, so the
/// two disagreed and the divider slid away from the pointer as it moved.
/// Both now use the span the bar's *centre* travels, `(w − bar)`, and a
/// press on the bar keeps where on it the pointer landed.
pub struct SplitPane<A: Widget, B: Widget, const VERTICAL: bool = false> {
    pub first: A,
    pub second: B,
    /// Where the divider sits, as a fraction of the span it travels.
    split: f32,
    /// The smallest a pane may be dragged, in px — M57/B46. It was a
    /// fraction (8%), which is 30 px in a small pane and 150 in a wide
    /// one. **Not zero**: a pane with no area cannot be clicked to get
    /// it back, so the control would become one-way.
    pub min_px: f32,
    /// Laid-out length along the axis, from the last frame — `on_event`
    /// sees `local` only. `Slider::track_w` says why.
    length: f32,
    /// Where on the bar the press landed, from its centre.
    grab: f32,
}

/// A split with its panes stacked, and a horizontal divider.
pub type VSplitPane<A, B> = SplitPane<A, B, true>;

impl<A: Widget + Default, B: Widget + Default, const V: bool> Default for SplitPane<A, B, V> {
    fn default() -> Self {
        SplitPane::new(A::default(), B::default())
    }
}

impl<A: Widget, B: Widget, const V: bool> SplitPane<A, B, V> {
    const FIRST: usize = 1;
    const SECOND: usize = 1 + A::NODE_COUNT;
    /// The divider's thickness, and the grab margin either side of it.
    const BAR: f32 = 6.0;

    pub fn new(first: A, second: B) -> Self {
        SplitPane { first, second, split: 0.5, min_px: 48.0, length: 0.0, grab: 0.0 }
    }

    pub fn split(&self) -> f32 {
        self.split
    }

    /// The fractions `min_px` allows at the last laid-out length.
    fn bounds(&self) -> (f32, f32) {
        let span = self.length - Self::BAR;
        if span <= 0.0 {
            // Not laid out yet: no size to take `min_px` from, so the old
            // 8% stop keeps it off the edges until a layout re-clamps it.
            return (0.08, 0.92);
        }
        let m = (self.min_px / span).min(0.5);
        (m, 1.0 - m)
    }

    /// Move the divider, clamped to leave both panes `min_px`.
    ///
    /// **A non-finite fraction is refused** and answers `false`
    /// (M55/T55.7, R16-F11): `num::clamp` passes a NaN through, so this
    /// stored one and `split()` reported it.
    pub fn set_split(&mut self, f: f32) -> bool {
        if !f.is_finite() {
            return false;
        }
        let (lo, hi) = self.bounds();
        let f = lumen_static::num::clamp(f, lo, hi);
        let moved = f != self.split;
        self.split = f;
        moved
    }

    /// The pointer's position along the axis.
    fn along(p: Point) -> f32 {
        if V { p.y } else { p.x }
    }

    /// The bar's leading edge, from the start of the pane.
    fn bar_at(&self, length: f32) -> f32 {
        ((length - Self::BAR) * self.split).max(0.0)
    }
}

pub struct SplitTable<A, B, const V: bool>(core::marker::PhantomData<(A, B)>);

impl<A: Widget, B: Widget, const V: bool> SplitTable<A, B, V> {
    const N: usize = 1 + A::NODE_COUNT + B::NODE_COUNT;
    const ARRAY: [NodeDesc; MAX_SPLIT_NODES] = {
        assert!(Self::N <= MAX_SPLIT_NODES, "SplitPane exceeds MAX_SPLIT_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SPLIT_NODES];
        // Focusable (M57/A12): its keyboard arms were dead code while nothing could focus it.
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER | NodeDesc::FOCUSABLE);
        lumen_static::widget::splice_child(&mut t, 1, A::SUB_TABLE, 0);
        let second = 1 + A::NODE_COUNT;
        lumen_static::widget::splice_child(&mut t, second, B::SUB_TABLE, 0);
        t[1].next_sibling = second as u32;
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_SPLIT_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SPLIT_NODES];
        s[0] = Seed::new(
            <SplitPane<A, B, V> as Widget>::KIND,
            <SplitPane<A, B, V> as Widget>::type_defaults, Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::BACKGROUND,
            Fields::BORDER,
            Fields::COLOR,
            Fields::CURSOR,
            Fields::VISIBLE,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Border));
        lumen_static::widget::splice_seeds(&mut s, 1, A::SEEDS);
        lumen_static::widget::splice_seeds(&mut s, 1 + A::NODE_COUNT, B::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<A: Widget, B: Widget, const V: bool> Widget for SplitPane<A, B, V> {
    const NODE_COUNT: usize = SplitTable::<A, B, V>::N;
    const DEPTH: usize = 1 + if A::DEPTH > B::DEPTH { A::DEPTH } else { B::DEPTH };
    /// **Rigid.** A split pane's size is its own; moving the divider
    /// resizes its panes and never itself.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = SplitTable::<A, B, V>::SLICE;
    const KIND: &'static str = if V { "vsplitpane" } else { "splitpane" };
    const SEEDS: &'static [Seed] = SplitTable::<A, B, V>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::FILL,
            color: Color(0xc6, 0xcd, 0xd8, 0xff),
            cursor: if V { lumen_static::style::Cursor::RowResize } else { lumen_static::style::Cursor::ColResize },
            ..Style::default()
        }
    }
}

impl<A: Widget, B: Widget, const V: bool> Node for SplitPane<A, B, V> {
    lumen_static::node_count!();
    /// Arrows move a divider, so they never leave this control
    /// (M44/T44.1; [`Node::owns_arrows`]).
    fn owns_arrows(&self) -> bool {
        true
    }

    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: limits.max_h },
        );
        self.length = if V { s.h } else { s.w };
        // Re-clamp at the size it now has: a window that shrank must not
        // leave a pane under `min_px`.
        self.set_split(self.split);
        let first = self.bar_at(self.length);
        let second = (self.length - Self::BAR - first).max(0.0);
        // Each pane is given exactly its share — fixed on the main axis,
        // so a `FILL` child fills its pane and not the pane next to it.
        let pane = |l: f32| {
            if V {
                Constraints { min_w: s.w, max_w: s.w, min_h: l, max_h: l }
            } else {
                Constraints { min_w: l, max_w: l, min_h: s.h, max_h: s.h }
            }
        };
        self.first.layout(cx, pane(first), style, st, i + Self::FIRST);
        self.second.layout(cx, pane(second), style, st, i + Self::SECOND);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let d = self.bar_at(self.length) + Self::BAR;
        self.first.place(origin, style, st, i + Self::FIRST);
        let at = if V { Point { x: origin.x, y: origin.y + d } } else { Point { x: origin.x + d, y: origin.y } };
        self.second.place(at, style, st, i + Self::SECOND);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius);
        self.first.paint(st, i + Self::FIRST, out);
        self.second.paint(st, i + Self::SECOND, out);
        // The divider last, so it is visible over either pane.
        let b = self.bar_at(self.length);
        let bar = if V {
            [r[0], r[1] + b, r[2], r[1] + b + Self::BAR]
        } else {
            [r[0] + b, r[1], r[0] + b + Self::BAR, r[3]]
        };
        out.rect(bar, s.fg);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        let r = lumen_static::ops::rect_of(&st[i]);
        let along = if V { y - r[1] } else { x - r[0] };
        let bar = self.bar_at(self.length);
        // **The divider is claimed first and with a margin.** A 6 px
        // target is under every accessibility floor there is; the grab
        // area is three times the bar it draws, which is what makes it
        // usable and is why this is not just `contains` on the bar rect.
        if along >= bar - Self::BAR && along <= bar + Self::BAR * 2.0 {
            return Some(lumen_static::Hit {
                node: i as u32,
                inner: None,
                local: Point { x: x - r[0], y: y - r[1] },
            });
        }
        self.first
            .hit(st, i + Self::FIRST, x, y)
            .or_else(|| self.second.hit(st, i + Self::SECOND, x, y))
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        // **Only what was aimed at the divider** — M57/A4. `hit` returns
        // this node only inside the grab zone; anything else reaching here
        // bubbled from a pane (a label that declined a press, a field that
        // declined an arrow) and moved the divider to wherever that child's
        // coordinates pointed (`docs/widget-audit.md` X2).
        if cx.target != cx.node {
            return false;
        }
        let span = (self.length - Self::BAR).max(1.0);
        let centre = self.bar_at(self.length) + Self::BAR / 2.0;
        let (back, fwd) = if V { (Key::Up, Key::Down) } else { (Key::Left, Key::Right) };
        match ev {
            Event::PointerDown { .. } => {
                // Where on the bar it was grabbed; a press in the margin
                // beside it grabs its nearest edge.
                self.grab = lumen_static::num::clamp(
                    Self::along(cx.local) - centre,
                    -Self::BAR * 1.5,
                    Self::BAR * 1.5,
                );
                // True even when it did not move: the gesture is this
                // node's, and declining mid-drag would hand it to a
                // parent halfway through.
                true
            }
            Event::PointerMove { .. } => {
                let at = Self::along(cx.local) - self.grab - Self::BAR / 2.0;
                self.set_split(at / span);
                true
            }
            Event::Key { key, down: true } if *key == back => self.set_split(self.split - 0.05),
            Event::Key { key, down: true } if *key == fwd => self.set_split(self.split + 0.05),
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            0 => self,
            _ if k < Self::SECOND => self.first.node(k - Self::FIRST),
            _ => self.second.node(k - Self::SECOND),
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            0 => self,
            _ if k < Self::SECOND => self.first.node_mut(k - Self::FIRST),
            _ => self.second.node_mut(k - Self::SECOND),
        }
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Splitter);
        // **A readable value as well as a numeric one.** A drive reads
        // `value`; `set_numeric_value` is for a screen reader's own
        // formatting. Publishing only the second is what `RangeSlider`
        // had to fix in T27.5, and this had the same gap.
        node.set_value(alloc::format!("{:.2}", self.split));
        node.set_numeric_value(self.split as f64);
    }
}

impl<A, B, S, const V: bool> lumen_static::event::Handlers<S> for SplitPane<A, B, V>
where
    A: Widget + lumen_static::event::Handlers<S>,
    B: Widget + lumen_static::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        if sel.i < Self::SECOND {
            self.first
                .handler(lumen_static::event::NodeSel { i: sel.i - Self::FIRST, inner: sel.inner })
        } else {
            self.second.handler(lumen_static::event::NodeSel {
                i: sel.i - Self::SECOND,
                inner: sel.inner,
            })
        }
    }
}

// ---- Collapsible -----------------------------------------------------

/// A header with a body that hides.
///
/// **The header is this node's own paint**, not a child: it is a title
/// and a chevron, and making it a widget would put two nodes where one
/// does, for a strip of text nobody styles separately. The body *is* a
/// child, because it is whatever the author puts in it.
pub struct Collapsible<W: Widget, S: TextStorage = &'static str> {
    pub title: S,
    pub body: W,
    open: bool,
    /// The header's height from the last layout, so `on_event` can tell
    /// a press on the header from one on the body.
    header_h: f32,
}

impl<W: Widget + Default, S: TextStorage + Default> Default for Collapsible<W, S> {
    fn default() -> Self {
        Collapsible { title: S::default(), body: W::default(), open: false, header_h: 0.0 }
    }
}

impl<W: Widget, S: TextStorage> Collapsible<W, S> {
    const BODY: usize = 1;
    /// The header's height.
    const HEADER: f32 = 40.0;
    /// Space under an open body.
    const BODY_BOTTOM: f32 = 12.0;

    pub fn new(title: S, body: W) -> Self {
        Collapsible { title, body, open: false, header_h: Self::HEADER }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_open(&mut self, open: bool) -> bool {
        let moved = open != self.open;
        self.open = open;
        moved
    }

    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        true
    }
}

pub struct CollapsibleTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> CollapsibleTable<W> {
    const ARRAY: [NodeDesc; MAX_SPLIT_NODES] = {
        assert!(W::NODE_COUNT < MAX_SPLIT_NODES, "Collapsible body exceeds MAX_SPLIT_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SPLIT_NODES];
        // Focusable (M57/A12): its keyboard arms were dead code while nothing could focus it.
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER | NodeDesc::FOCUSABLE);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_SPLIT_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SPLIT_NODES];
        s[0] = Seed::new("collapsible", collapsible_defaults, Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::PADDING,
            Fields::BACKGROUND,
            Fields::BORDER,
            Fields::BACKGROUND_FILL,
            Fields::COLOR,
            Fields::FONT_SIZE,
            Fields::FONT_WEIGHT,
            Fields::RADIUS,
            Fields::CURSOR,
            Fields::VISIBLE,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text).background(lumen_static::style::Role::Surface));
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

/// Shared by the seed and by `type_defaults`, because a `Seed` takes a
/// plain `fn` and a generic one is not that.
fn collapsible_defaults() -> Style {
    Style {
        width: Length::FILL,
        height: Length::Shrink,
        padding: Edges { left: 14.0, right: 14.0, top: 0.0, bottom: 0.0 },
        background: Color(0xf3, 0xf5, 0xf9, 0xff),
        color: Color(0x30, 0x3a, 0x4c, 0xff),
        font_size: 15.0,
        font_weight: 600.0,
        radius: 8.0,
        cursor: lumen_static::style::Cursor::Pointer,
        ..Style::default()
    }
}

impl<W: Widget, S: TextStorage> Widget for Collapsible<W, S> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// **Not rigid**: opening one changes its height, and the column
    /// above it has to know.
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = CollapsibleTable::<W>::SLICE;
    const KIND: &'static str = "collapsible";
    const SEEDS: &'static [Seed] = CollapsibleTable::<W>::SEED_SLICE;

    fn type_defaults() -> Style {
        collapsible_defaults()
    }
}

impl<W: Widget, S: TextStorage> Node for Collapsible<W, S> {
    lumen_static::node_count!();
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        self.header_h = Self::HEADER;
        let body = if self.open {
            let inner = Constraints {
                min_w: 0.0,
                max_w: (limits.max_w - me.padding.horizontal()).max(0.0),
                min_h: 0.0,
                max_h: limits.max_h,
            };
            self.body.layout(cx, inner, style, st, i + Self::BODY)
        } else {
            // **Closed is absent.** The body is zeroed, so it holds no
            // geometry for `paint` or `hit` — the same reason a shut
            // `Veil` is zeroed, and what stops a collapsed section
            // taking clicks through the one below it.
            lumen_static::ops::hide_subtree(st, i + Self::BODY, limits, W::NODE_COUNT)
        };
        // 8 px above the body and `BODY_BOTTOM` below it (M57/B47): the
        // open body sat flush against the section's bottom edge.
        let h = self.header_h + if self.open { body.h + 8.0 + Self::BODY_BOTTOM } else { 0.0 };
        let s = lumen_static::ops::resolve_self(limits, &me, Size { w: limits.max_w, h });
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        if !self.open {
            return;
        }
        let p = style[i].padding;
        self.body.place(
            Point { x: origin.x + p.left, y: origin.y + self.header_h + 8.0 },
            style,
            st,
            i + Self::BODY,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        lumen_static::ops::fill(st, i, out);
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        let p = s.pad;
        let mid = r[1] + self.header_h / 2.0;
        out.text_weighted(
            [r[0] + p.0, mid - s.font_size * 0.6, r[2] - p.2 - 20.0, r[3]],
            self.title.as_ref(),
            s.font_size,
            s.font_weight,
            s.fg,
        );
        // A chevron: down when open, right when shut. Two lines, so it
        // needs no glyph and no font — `Icon`'s argument.
        let cx0 = r[2] - p.2 - 8.0;
        let k = 4.0;
        if self.open {
            out.line(
                Point { x: cx0 - k, y: mid - k / 2.0 },
                Point { x: cx0, y: mid + k / 2.0 },
                2.0,
                s.fg,
            );
            out.line(
                Point { x: cx0, y: mid + k / 2.0 },
                Point { x: cx0 + k, y: mid - k / 2.0 },
                2.0,
                s.fg,
            );
        } else {
            out.line(
                Point { x: cx0 - k / 2.0, y: mid - k },
                Point { x: cx0 + k / 2.0, y: mid },
                2.0,
                s.fg,
            );
            out.line(
                Point { x: cx0 + k / 2.0, y: mid },
                Point { x: cx0 - k / 2.0, y: mid + k },
                2.0,
                s.fg,
            );
        }
        if self.open {
            self.body.paint(st, i + Self::BODY, out);
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // The body first, where it exists — what is *in* a section is
        // above the section.
        if self.open {
            if let Some(h) = self.body.hit(st, i + Self::BODY, x, y) {
                return Some(h);
            }
        }
        Some(lumen_static::Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // **Only a press on the header toggles**: one aimed at this
            // node (not bubbled from the body) and inside the header band.
            // `hit` gives the body first refusal, but a `Label` in the body
            // *is* hit and declines, and its press arrived here — in the
            // label's own coordinates, where `y = 8` read as a header
            // press, so clicking an open section's text collapsed it
            // (M57/A4, `docs/widget-audit.md` X2).
            Event::PointerDown { .. } if cx.target == cx.node && cx.local.y <= self.header_h => self.toggle(),
            // Enter and Space, as on any button-like header (M57/B47: only
            // Enter toggled).
            Event::Key { key: Key::Enter | Key::Space, down: true } if cx.target == cx.node => self.toggle(),
            // A screen reader's open and close (M54/T54.4): each only
            // when it changes something.
            Event::Action { action: lumen_static::Action::Expand, .. } if !self.open => self.toggle(),
            Event::Action { action: lumen_static::Action::Collapse, .. } if self.open => self.toggle(),
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == 0 {
            self
        } else {
            self.body.node(k - Self::BODY)
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == 0 {
            self
        } else {
            self.body.node_mut(k - Self::BODY)
        }
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::DisclosureTriangle);
        node.set_label(self.title.as_ref());
        node.set_expanded(self.open);
        node.add_action(if self.open { accesskit::Action::Collapse } else { accesskit::Action::Expand });
        node.set_value(if self.open { "expanded" } else { "collapsed" });
    }
}

impl<W, S, A> lumen_static::event::Handlers<A> for Collapsible<W, S>
where
    W: Widget + lumen_static::event::Handlers<A>,
    S: TextStorage,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut A, &EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.body
            .handler(lumen_static::event::NodeSel { i: sel.i - Self::BODY, inner: sel.inner })
    }
}

// ---- PaneGrid --------------------------------------------------------

/// Four panes and three draggable dividers.
///
/// # Why four and not a tree
///
/// iced's `PaneGrid` is a *tree* of splits the user can re-arrange at
/// runtime: split a pane, drop one onto another, close one. That is
/// topology as data, and this framework's topology is the type — the
/// one thing it does not do. What it *can* do is the shape almost every
/// pane grid actually ends up in: a 2×2 with a shared vertical divider
/// and a horizontal one per column, all three draggable.
///
/// So this is the honest static analogue, and the part that is missing
/// is named rather than faked: **panes cannot be added, removed or
/// swapped at runtime.** An application that needs that needs a
/// different tree, which in this architecture means a different type —
/// and `Either` is how one is chosen (M24/T24.3).
///
/// `SplitPane<SplitPane<A, C>, SplitPane<B, D>>` is the same four panes
/// and *almost* the same widget, with one difference that matters: its
/// two horizontal dividers are inside separate subtrees, so nothing can
/// keep them in step or report them together. A pane grid is a thing
/// with three dividers, not a tree with one.
pub struct PaneGrid<A: Widget, B: Widget, C: Widget, D: Widget> {
    pub top_left: A,
    pub top_right: B,
    pub bottom_left: C,
    pub bottom_right: D,
    /// The shared vertical divider, as a fraction of the width.
    v: f32,
    /// Each column's horizontal divider, as a fraction of the height.
    left: f32,
    right: f32,
    /// Which divider a drag is on: 0 vertical, 1 left, 2 right.
    grabbed: u8,
    /// Which divider the keyboard moves — M57/B48. Tab walks 0 → 1 → 2
    /// and then lets focus go, as `RangeSlider`'s two handles do (B25);
    /// Left/Right move the vertical one, Up/Down a horizontal one.
    kbd: u8,
    /// Where on the grabbed divider the press landed, from its centre.
    grab: f32,
    /// The smallest a pane may be dragged, in px — M57/B48, as
    /// `SplitPane::min_px`.
    pub min_px: f32,
    size: Size,
}

impl<A, B, C, D> Default for PaneGrid<A, B, C, D>
where
    A: Widget + Default,
    B: Widget + Default,
    C: Widget + Default,
    D: Widget + Default,
{
    fn default() -> Self {
        PaneGrid {
            top_left: A::default(),
            top_right: B::default(),
            bottom_left: C::default(),
            bottom_right: D::default(),
            v: 0.5,
            left: 0.5,
            right: 0.5,
            grabbed: 0,
            kbd: 0,
            grab: 0.0,
            min_px: 48.0,
            size: Size { w: 0.0, h: 0.0 },
        }
    }
}

impl<A: Widget, B: Widget, C: Widget, D: Widget> PaneGrid<A, B, C, D> {
    const TL: usize = 1;
    const TR: usize = 1 + A::NODE_COUNT;
    const BL: usize = 1 + A::NODE_COUNT + B::NODE_COUNT;
    const BR: usize = 1 + A::NODE_COUNT + B::NODE_COUNT + C::NODE_COUNT;
    const BAR: f32 = 6.0;

    pub fn new(top_left: A, top_right: B, bottom_left: C, bottom_right: D) -> Self {
        PaneGrid {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
            v: 0.5,
            left: 0.5,
            right: 0.5,
            grabbed: 0,
            kbd: 0,
            grab: 0.0,
            min_px: 48.0,
            size: Size { w: 0.0, h: 0.0 },
        }
    }

    /// The vertical divider's fraction.
    pub fn vertical(&self) -> f32 {
        self.v
    }

    /// A column's horizontal divider: 0 for the left, anything else the
    /// right.
    pub fn horizontal(&self, column: usize) -> f32 {
        if column == 0 {
            self.left
        } else {
            self.right
        }
    }

    /// Move one divider, clamped away from both edges. `which` is 0 for
    /// the vertical, 1 for the left column's, 2 for the right's.
    ///
    /// Clamped to leave each pane `min_px` (M57/B48; it was an 8% stop):
    /// a pane dragged to zero area cannot be clicked to get it back.
    pub fn set_divider(&mut self, which: u8, f: f32) -> bool {
        let len = if which == 0 { self.size.w } else { self.size.h } - Self::BAR;
        // Before a layout there is no size to take `min_px` from: keep the
        // old 8% stop, so no divider can reach an edge even then.
        let m = if len > 0.0 { (self.min_px / len).min(0.5) } else { 0.08 };
        let f = lumen_static::num::clamp(f, m, 1.0 - m);
        let slot = match which {
            0 => &mut self.v,
            1 => &mut self.left,
            _ => &mut self.right,
        };
        let moved = *slot != f;
        *slot = f;
        moved
    }

    /// The divider under a local point, if any — the same grab margin
    /// `SplitPane` uses, which is three times the bar it draws.
    fn divider_at(&self, x: f32, y: f32) -> Option<u8> {
        let (w, h) = (self.size.w, self.size.h);
        let vx = (w - Self::BAR) * self.v;
        if x >= vx - Self::BAR && x <= vx + Self::BAR * 2.0 {
            return Some(0);
        }
        let (which, frac) = if x < vx { (1u8, self.left) } else { (2, self.right) };
        let hy = (h - Self::BAR) * frac;
        if y >= hy - Self::BAR && y <= hy + Self::BAR * 2.0 {
            return Some(which);
        }
        None
    }
}

pub struct PaneTable<A, B, C, D>(core::marker::PhantomData<(A, B, C, D)>);

impl<A: Widget, B: Widget, C: Widget, D: Widget> PaneTable<A, B, C, D> {
    const N: usize =
        1 + A::NODE_COUNT + B::NODE_COUNT + C::NODE_COUNT + D::NODE_COUNT;
    const ARRAY: [NodeDesc; MAX_SPLIT_NODES] = {
        assert!(Self::N <= MAX_SPLIT_NODES, "PaneGrid exceeds MAX_SPLIT_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SPLIT_NODES];
        // Focusable (M57/A12): its keyboard arms were dead code while nothing could focus it.
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 4, NodeDesc::CONTAINER | NodeDesc::FOCUSABLE);
        lumen_static::widget::splice_child(&mut t, 1, A::SUB_TABLE, 0);
        lumen_static::widget::splice_child(&mut t, 1 + A::NODE_COUNT, B::SUB_TABLE, 0);
        lumen_static::widget::splice_child(
            &mut t,
            1 + A::NODE_COUNT + B::NODE_COUNT,
            C::SUB_TABLE,
            0,
        );
        lumen_static::widget::splice_child(
            &mut t,
            1 + A::NODE_COUNT + B::NODE_COUNT + C::NODE_COUNT,
            D::SUB_TABLE,
            0,
        );
        // **The sibling chain, which `splice_child` cannot know.** It
        // writes one child's subtree; only the parent knows what comes
        // after it, and a table whose children do not point at each
        // other fails `table_is_well_formed` — at compile time, which
        // is where this was caught.
        let (tr, bl, br) = (
            1 + A::NODE_COUNT,
            1 + A::NODE_COUNT + B::NODE_COUNT,
            1 + A::NODE_COUNT + B::NODE_COUNT + C::NODE_COUNT,
        );
        t[1].next_sibling = tr as u32;
        t[tr].next_sibling = bl as u32;
        t[bl].next_sibling = br as u32;
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_SPLIT_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SPLIT_NODES];
        s[0] = Seed::new(
            <PaneGrid<A, B, C, D> as Widget>::KIND,
            <PaneGrid<A, B, C, D> as Widget>::type_defaults, Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::BACKGROUND,
            Fields::BORDER,
            Fields::COLOR,
            Fields::RADIUS,
            Fields::VISIBLE,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Border).background(lumen_static::style::Role::Raised));
        lumen_static::widget::splice_seeds(&mut s, 1, A::SEEDS);
        lumen_static::widget::splice_seeds(&mut s, 1 + A::NODE_COUNT, B::SEEDS);
        lumen_static::widget::splice_seeds(&mut s, 1 + A::NODE_COUNT + B::NODE_COUNT, C::SEEDS);
        lumen_static::widget::splice_seeds(
            &mut s,
            1 + A::NODE_COUNT + B::NODE_COUNT + C::NODE_COUNT,
            D::SEEDS,
        );
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<A: Widget, B: Widget, C: Widget, D: Widget> Widget for PaneGrid<A, B, C, D> {
    const NODE_COUNT: usize = PaneTable::<A, B, C, D>::N;
    const DEPTH: usize = 1 + max4(A::DEPTH, B::DEPTH, C::DEPTH, D::DEPTH);
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = PaneTable::<A, B, C, D>::SLICE;
    const KIND: &'static str = "panegrid";
    const SEEDS: &'static [Seed] = PaneTable::<A, B, C, D>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::FILL,
            background: Color(0xf3, 0xf5, 0xf9, 0xff),
            color: Color(0xcb, 0xd2, 0xdb, 0xff),
            ..Style::default()
        }
    }
}

const fn max4(a: usize, b: usize, c: usize, d: usize) -> usize {
    let ab = if a > b { a } else { b };
    let cd = if c > d { c } else { d };
    if ab > cd {
        ab
    } else {
        cd
    }
}

impl<A, B, C, D> Node for PaneGrid<A, B, C, D>
where
    A: Widget + Node,
    B: Widget + Node,
    C: Widget + Node,
    D: Widget + Node,
{
    lumen_static::node_count!();
    /// Arrows move a divider, so they never leave this control
    /// (M44/T44.1; [`Node::owns_arrows`]).
    fn owns_arrows(&self) -> bool {
        true
    }

    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: limits.max_h },
        );
        self.size = s;
        // Re-clamp at the size it now has (M57/B48): a window that shrank
        // must not leave a pane under `min_px`.
        let (v, l, r) = (self.v, self.left, self.right);
        self.set_divider(0, v);
        self.set_divider(1, l);
        self.set_divider(2, r);
        let bar = Self::BAR;
        let lw = ((s.w - bar) * self.v).max(0.0);
        let rw = (s.w - bar - lw).max(0.0);
        let lh = ((s.h - bar) * self.left).max(0.0);
        let rh = ((s.h - bar) * self.right).max(0.0);
        // Each pane is given exactly its share, fixed on both axes: a
        // `FILL` child fills its pane and not the one beside it, which
        // is `SplitPane`'s rule in two dimensions.
        let pane = |w: f32, h: f32| Constraints { min_w: w, max_w: w, min_h: h, max_h: h };
        self.top_left.layout(cx, pane(lw, lh), style, st, i + Self::TL);
        self.top_right.layout(cx, pane(rw, rh), style, st, i + Self::TR);
        self.bottom_left
            .layout(cx, pane(lw, (s.h - bar - lh).max(0.0)), style, st, i + Self::BL);
        self.bottom_right
            .layout(cx, pane(rw, (s.h - bar - rh).max(0.0)), style, st, i + Self::BR);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let (w, h) = (st[i].size.w, st[i].size.h);
        let bar = Self::BAR;
        let lw = ((w - bar) * self.v).max(0.0);
        let lh = ((h - bar) * self.left).max(0.0);
        let rh = ((h - bar) * self.right).max(0.0);
        let right_x = origin.x + lw + bar;
        self.top_left.place(origin, style, st, i + Self::TL);
        self.top_right.place(Point { x: right_x, y: origin.y }, style, st, i + Self::TR);
        self.bottom_left.place(
            Point { x: origin.x, y: origin.y + lh + bar },
            style,
            st,
            i + Self::BL,
        );
        self.bottom_right.place(
            Point { x: right_x, y: origin.y + rh + bar },
            style,
            st,
            i + Self::BR,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius);
        self.top_left.paint(st, i + Self::TL, out);
        self.top_right.paint(st, i + Self::TR, out);
        self.bottom_left.paint(st, i + Self::BL, out);
        self.bottom_right.paint(st, i + Self::BR, out);
        // The dividers last, over the panes. The horizontal ones stop
        // at the vertical: they belong to their column, which is the
        // whole difference between this and two nested splits.
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        let vx = r[0] + (w - Self::BAR) * self.v;
        out.rect([vx, r[1], vx + Self::BAR, r[3]], s.fg);
        let ly = r[1] + (h - Self::BAR) * self.left;
        out.rect([r[0], ly, vx, ly + Self::BAR], s.fg);
        let ry = r[1] + (h - Self::BAR) * self.right;
        out.rect([vx + Self::BAR, ry, r[2], ry + Self::BAR], s.fg);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        let r = lumen_static::ops::rect_of(&st[i]);
        let local = Point { x: x - r[0], y: y - r[1] };
        if self.divider_at(local.x, local.y).is_some() {
            return Some(lumen_static::Hit { node: i as u32, inner: None, local });
        }
        self.top_left
            .hit(st, i + Self::TL, x, y)
            .or_else(|| self.top_right.hit(st, i + Self::TR, x, y))
            .or_else(|| self.bottom_left.hit(st, i + Self::BL, x, y))
            .or_else(|| self.bottom_right.hit(st, i + Self::BR, x, y))
    }

    /// Column-resize over the vertical divider, row-resize over the
    /// horizontal ones (M57/B48): one style cursor cannot say both.
    fn cursor_at(&self, local: Point) -> Option<lumen_static::style::Cursor> {
        match self.divider_at(local.x, local.y) {
            Some(0) => Some(lumen_static::style::Cursor::ColResize),
            Some(_) => Some(lumen_static::style::Cursor::RowResize),
            None => None,
        }
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // **Which divider is decided on the press and kept.** A
            // drag that re-decided on every move would hand the gesture
            // to whichever divider the pointer wandered near — and the
            // vertical one's grab margin crosses both horizontals at
            // their intersection, so that is not a rare case.
            // **A press not on a divider is not this node's** — M57/A4.
            // It was `divider_at(..).unwrap_or(0)`: a press that bubbled
            // up from a pane's label found no divider in the label's
            // coordinates and dragged the vertical one to wherever those
            // coordinates pointed (`docs/widget-audit.md` X2).
            Event::PointerDown { .. } => {
                if cx.target != cx.node {
                    return false;
                }
                let Some(d) = self.divider_at(cx.local.x, cx.local.y) else { return false };
                self.grabbed = d;
                let along = if d == 0 { cx.local.x } else { cx.local.y };
                self.grab = lumen_static::num::clamp(along - self.centre(d), -Self::BAR * 1.5, Self::BAR * 1.5);
                true
            }
            Event::PointerMove { .. } => {
                self.drag(cx);
                true
            }
            // **Each divider along its own axis** — M57/B48: only the
            // vertical one had keys. Tab walks the dividers, then lets
            // focus go at either end, as a range slider's handles do.
            Event::Key { key: Key::Tab, down: true } if cx.target == cx.node => {
                let back = cx.mods.has(lumen_static::event::Mods::SHIFT);
                match (back, self.kbd) {
                    (false, k) if k < 2 => {
                        self.kbd += 1;
                        true
                    }
                    (true, k) if k > 0 => {
                        self.kbd -= 1;
                        true
                    }
                    _ => false,
                }
            }
            Event::Key { key: Key::Left | Key::Right, down: true } if cx.target == cx.node && self.kbd == 0 => {
                let d = if matches!(ev, Event::Key { key: Key::Left, .. }) { -0.05 } else { 0.05 };
                self.set_divider(0, self.v + d)
            }
            Event::Key { key: Key::Up | Key::Down, down: true } if cx.target == cx.node && self.kbd > 0 => {
                let d = if matches!(ev, Event::Key { key: Key::Up, .. }) { -0.05 } else { 0.05 };
                let f = if self.kbd == 1 { self.left } else { self.right };
                self.set_divider(self.kbd, f + d)
            }
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == 0 {
            self
        } else if k < Self::TR {
            self.top_left.node(k - Self::TL)
        } else if k < Self::BL {
            self.top_right.node(k - Self::TR)
        } else if k < Self::BR {
            self.bottom_left.node(k - Self::BL)
        } else {
            self.bottom_right.node(k - Self::BR)
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == 0 {
            self
        } else if k < Self::TR {
            self.top_left.node_mut(k - Self::TL)
        } else if k < Self::BL {
            self.top_right.node_mut(k - Self::TR)
        } else if k < Self::BR {
            self.bottom_left.node_mut(k - Self::BL)
        } else {
            self.bottom_right.node_mut(k - Self::BR)
        }
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Group);
        // The three fractions, so a drive can assert a drag without
        // reading pixels.
        node.set_value(alloc::format!(
            "{:.2},{:.2},{:.2}",
            self.v,
            self.left,
            self.right
        ));
    }
}

impl<A: Widget, B: Widget, C: Widget, D: Widget> PaneGrid<A, B, C, D> {
    /// The grabbed divider's centre along its axis.
    fn centre(&self, which: u8) -> f32 {
        let (len, f) = match which {
            0 => (self.size.w, self.v),
            1 => (self.size.h, self.left),
            _ => (self.size.h, self.right),
        };
        (len - Self::BAR) * f + Self::BAR / 2.0
    }

    /// Follow the pointer, keeping the grab — `SplitPane`'s mapping
    /// (M57/B46): the bar's centre travels `(len − bar)`, and `x / len`
    /// against it drifted.
    fn drag(&mut self, cx: &EventCtx) {
        let (along, len) = if self.grabbed == 0 { (cx.local.x, self.size.w) } else { (cx.local.y, self.size.h) };
        let span = (len - Self::BAR).max(1.0);
        self.set_divider(self.grabbed, (along - self.grab - Self::BAR / 2.0) / span);
    }
}

impl<A: Widget, B: Widget, C: Widget, D: Widget, S> lumen_static::event::Handlers<S>
    for PaneGrid<A, B, C, D>
{
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}
