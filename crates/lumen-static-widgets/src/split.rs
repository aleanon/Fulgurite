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

/// Two panes with a draggable divider between them.
pub struct SplitPane<A: Widget, B: Widget> {
    pub first: A,
    pub second: B,
    /// Where the divider sits, as a fraction of the width.
    split: f32,
    /// Laid-out width, from the last frame — `on_event` sees `local`
    /// only. `Slider::track_w` says why.
    width: f32,
}

impl<A: Widget + Default, B: Widget + Default> Default for SplitPane<A, B> {
    fn default() -> Self {
        SplitPane { first: A::default(), second: B::default(), split: 0.5, width: 0.0 }
    }
}

impl<A: Widget, B: Widget> SplitPane<A, B> {
    const FIRST: usize = 1;
    const SECOND: usize = 1 + A::NODE_COUNT;
    /// The divider's width, and the grab margin either side of it.
    const BAR: f32 = 6.0;
    /// How close to an edge the divider may be dragged.
    ///
    /// **Not zero.** A divider dragged flush to an edge leaves a pane
    /// with no area, and a pane with no area cannot be clicked to get it
    /// back — the control becomes one-way. Every toolkit keeps a stop.
    const MIN: f32 = 0.08;

    pub fn new(first: A, second: B) -> Self {
        SplitPane { first, second, split: 0.5, width: 0.0 }
    }

    pub fn split(&self) -> f32 {
        self.split
    }

    /// Move the divider, clamped away from both edges.
    pub fn set_split(&mut self, f: f32) -> bool {
        let f = lumen_static::num::clamp(f, Self::MIN, 1.0 - Self::MIN);
        let moved = f != self.split;
        self.split = f;
        moved
    }
}

pub struct SplitTable<A, B>(core::marker::PhantomData<(A, B)>);

impl<A: Widget, B: Widget> SplitTable<A, B> {
    const N: usize = 1 + A::NODE_COUNT + B::NODE_COUNT;
    const ARRAY: [NodeDesc; MAX_SPLIT_NODES] = {
        assert!(Self::N <= MAX_SPLIT_NODES, "SplitPane exceeds MAX_SPLIT_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SPLIT_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
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
            <SplitPane<A, B> as Widget>::KIND,
            <SplitPane<A, B> as Widget>::type_defaults,
        )
        .reading(Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::BACKGROUND,
            Fields::COLOR,
            Fields::CURSOR,
            Fields::VISIBLE,
        ]));
        lumen_static::widget::splice_seeds(&mut s, 1, A::SEEDS);
        lumen_static::widget::splice_seeds(&mut s, 1 + A::NODE_COUNT, B::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<A: Widget, B: Widget> Widget for SplitPane<A, B> {
    const NODE_COUNT: usize = SplitTable::<A, B>::N;
    const DEPTH: usize = 1 + if A::DEPTH > B::DEPTH { A::DEPTH } else { B::DEPTH };
    /// **Rigid.** A split pane's size is its own; moving the divider
    /// resizes its panes and never itself.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = SplitTable::<A, B>::SLICE;
    const KIND: &'static str = "splitpane";
    const SEEDS: &'static [Seed] = SplitTable::<A, B>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::FILL,
            color: Color(0xc6, 0xcd, 0xd8, 0xff),
            cursor: lumen_static::style::Cursor::ColResize,
            ..Style::default()
        }
    }
}

impl<A: Widget, B: Widget> Node for SplitPane<A, B> {
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
        self.width = s.w;
        let bar = Self::BAR;
        let left = ((s.w - bar) * self.split).max(0.0);
        let right = (s.w - bar - left).max(0.0);
        // Each pane is given exactly its share — fixed on the main axis,
        // so a `FILL` child fills its pane and not the pane next to it.
        let pane = |w: f32| Constraints { min_w: w, max_w: w, min_h: s.h, max_h: s.h };
        self.first.layout(cx, pane(left), style, st, i + Self::FIRST);
        self.second.layout(cx, pane(right), style, st, i + Self::SECOND);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let w = st[i].size.w;
        let left = ((w - Self::BAR) * self.split).max(0.0);
        self.first.place(origin, style, st, i + Self::FIRST);
        self.second.place(
            Point { x: origin.x + left + Self::BAR, y: origin.y },
            style,
            st,
            i + Self::SECOND,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius);
        self.first.paint(st, i + Self::FIRST, out);
        self.second.paint(st, i + Self::SECOND, out);
        // The divider last, so it is visible over either pane.
        let x = r[0] + ((r[2] - r[0]) - Self::BAR) * self.split;
        out.rect([x, r[1], x + Self::BAR, r[3]], s.fg);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        let r = lumen_static::ops::rect_of(&st[i]);
        let bar = r[0] + ((r[2] - r[0]) - Self::BAR) * self.split;
        // **The divider is claimed first and with a margin.** A 6 px
        // target is under every accessibility floor there is; the grab
        // area is three times the bar it draws, which is what makes it
        // usable and is why this is not just `contains` on the bar rect.
        if x >= bar - Self::BAR && x <= bar + Self::BAR * 2.0 {
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
        let frac = |x: f32, w: f32| if w > 0.0 { x / w } else { 0.5 };
        match ev {
            Event::PointerDown { .. } | Event::PointerMove { .. } => {
                self.set_split(frac(cx.local.x, self.width));
                // True even when it did not move: the gesture is this
                // node's, and declining mid-drag would hand it to a
                // parent halfway through.
                true
            }
            Event::Key { key: Key::Left, down: true } => self.set_split(self.split - 0.05),
            Event::Key { key: Key::Right, down: true } => self.set_split(self.split + 0.05),
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

    #[cfg(feature = "observe")]
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

impl<A, B, S> lumen_static::event::Handlers<S> for SplitPane<A, B>
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
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_SPLIT_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SPLIT_NODES];
        s[0] = Seed::new("collapsible", collapsible_defaults).reading(Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::PADDING,
            Fields::BACKGROUND,
            Fields::COLOR,
            Fields::FONT_SIZE,
            Fields::FONT_WEIGHT,
            Fields::RADIUS,
            Fields::CURSOR,
            Fields::VISIBLE,
        ]));
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
        let h = self.header_h + if self.open { body.h + 8.0 } else { 0.0 };
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
            // **Only a press on the header toggles.** A press anywhere
            // in an open section would collapse it the moment anyone
            // clicked what is inside — and `hit` gives the body first
            // refusal, so a press arriving here with `local.y` past the
            // header is on the section's own padding, not on a control.
            Event::PointerDown { .. } if cx.local.y <= self.header_h => self.toggle(),
            Event::Key { key: Key::Enter, down: true } => self.toggle(),
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

    #[cfg(feature = "observe")]
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
