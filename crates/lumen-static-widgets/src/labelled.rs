//! A control with its label: a press on the words is a press on the
//! control — M57/B11 (`docs/plan-widget-fixes.md`).
//!
//! # Why a wrapper, and why the hit goes to the control
//!
//! The showcase put a `Label` beside each checkbox, switch and radio in a
//! row, and clicking "I agree" did nothing: the label was a separate node
//! with no handler, so the press stopped at the row
//! (`docs/widget-audit.md`, CheckBox). Every desktop toolkit makes a
//! control's label part of its target.
//!
//! `Labelled<W>` paints the text beside any control, and its
//! [`Node::hit`] answers a point on the label **with the control's own
//! node**. That one decision is the whole feature: the pass then hovers,
//! presses, focuses and activates the control exactly as if it had been
//! hit directly — its hover shade, its press shade, the release rule that
//! a click must begin and end on it (M57/A9) — with no second path to keep
//! in step. The control's type is unchanged, so a `Toggle` keeps its own
//! `on` and handler, and a `Labelled<Slider>` would work the same way.
//!
//! The label's text is `Role::Text` through the seed, at the kind's font
//! size, and it takes no events of its own.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::LayoutCx;

/// Upper bound on `1 + W::NODE_COUNT`, for the same reason as
/// [`crate::surface::MAX_SURFACE_NODES`].
pub const MAX_LABELLED_NODES: usize = 64;

/// The control's part id: `#owner > #labelled-control` addresses it.
pub const PART_CONTROL: &str = "labelled-control";

/// A control and the words that name it.
pub struct Labelled<W: Widget> {
    pub control: W,
    /// What it says. Beside the control, after it.
    pub label: &'static str,
    measured: lumen_static::text::FoldedText,
    /// The spacing the last layout put between control and label: none
    /// when there is no label.
    gap: f32,
}

impl<W: Widget + Default> Default for Labelled<W> {
    fn default() -> Self {
        Labelled::new(W::default(), "")
    }
}

impl<W: Widget> Labelled<W> {
    /// The control's index, relative to this node.
    const AT: usize = 1;

    pub fn new(control: W, label: &'static str) -> Self {
        Labelled { control, label, measured: lumen_static::text::FoldedText::new(), gap: 0.0 }
    }
}

pub struct LabelledTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> LabelledTable<W> {
    const ARRAY: [NodeDesc; MAX_LABELLED_NODES] = {
        assert!(W::NODE_COUNT < MAX_LABELLED_NODES, "Labelled control exceeds MAX_LABELLED_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_LABELLED_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        // The control is a part with a name, so a selector can reach it
        // past the wrapper: `#my_toggle > #labelled-control`.
        t[1] = t[1].with_id(PART_CONTROL);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_LABELLED_NODES] = {
        let mut s = [Seed::WIDGET; MAX_LABELLED_NODES];
        s[0] = Seed::new(
            <Labelled<W> as Widget>::KIND,
            <Labelled<W> as Widget>::type_defaults,
            Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::SPACING,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::FONT_WEIGHT,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]),
        )
        .with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text));
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

impl<W: Widget> Widget for Labelled<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// Its width follows its text.
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = LabelledTable::<W>::SLICE;
    const KIND: &'static str = "labelled";
    const SEEDS: &'static [Seed] = LabelledTable::<W>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            spacing: 10.0,
            color: Color(0x1b, 0x22, 0x30, 0xff),
            font_size: 14.0,
            // The words are part of the target, so they say so.
            cursor: lumen_static::style::Cursor::Pointer,
            ..Style::default()
        }
    }
}

impl<W: Widget> Labelled<W> {
    /// The label's rect, from the laid-out control.
    fn label_rect(&self, st: &[NodeState], i: usize) -> [f32; 4] {
        let me = &st[i];
        let c = &st[i + Self::AT];
        let m = self.measured.size();
        let x = c.pos.x + c.size.w + self.gap;
        let y = me.pos.y + (me.size.h - m.h).max(0.0) / 2.0;
        [x, y, x + m.w, y + m.h]
    }
}

impl<W: Widget> Node for Labelled<W> {
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
        // The control at its own size: its minimum is not inherited
        // (M27/T27.5's rule for a wrapper).
        let loose = Constraints { min_w: 0.0, min_h: 0.0, ..limits };
        let c = self.control.layout(cx, loose, style, st, i + Self::AT);
        let text = self.label;
        let key = lumen_static::text::MeasureKey::of(limits, &me, cx.scale());
        let room = (limits.max_w - c.w - me.spacing).max(0.0);
        let m = self.measured.get_or(key, text, || cx.text.measure_weighted(text, me.font_size, me.font_weight, room));
        self.gap = if text.is_empty() { 0.0 } else { me.spacing };
        let content = Size { w: c.w + self.gap + if text.is_empty() { 0.0 } else { m.w }, h: c.h.max(m.h) };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let ch = st[i + Self::AT].size.h;
        let dy = (st[i].size.h - ch).max(0.0) / 2.0;
        self.control.place(Point { x: origin.x, y: origin.y + dy }, style, st, i + Self::AT);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if st[i].size.w <= 0.0 || st[i].size.h <= 0.0 {
            return;
        }
        self.control.paint(st, i + Self::AT, out);
        if !self.label.is_empty() {
            let s = &st[i];
            out.text_weighted(self.label_rect(st, i), self.label, s.font_size, s.font_weight, s.fg);
        }
    }

    /// **A point on the label is a point on the control.** See the module
    /// docs: every consequence of a press follows from answering with the
    /// control's node, so nothing else here handles events.
    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        if let Some(h) = self.control.hit(st, i + Self::AT, x, y) {
            return Some(h);
        }
        let c = i + Self::AT;
        Some(lumen_static::Hit {
            node: c as u32,
            inner: None,
            local: Point { x: x - st[c].pos.x, y: y - st[c].pos.y },
        })
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == 0 { self } else { self.control.node(k - Self::AT) }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == 0 { self } else { self.control.node_mut(k - Self::AT) }
    }

    /// The words, published as what they are: a reader and the agent's
    /// `~text` selector find the control by them.
    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Label);
        node.set_value(self.label);
    }
}

impl<W, S> lumen_static::event::Handlers<S> for Labelled<W>
where
    W: Widget + lumen_static::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.control.handler(lumen_static::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}
