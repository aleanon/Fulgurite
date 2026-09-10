//! A panel that blurs what is behind it — M19/T19.1.
//!
//! `Glass<W>` is one node with one child. It emits [`Cmd::Glass`](crate::display::Cmd::Glass) over
//! its own rect and then paints its content on top, so a toolbar's
//! buttons sit *on* the frosted panel rather than under it.
//!
//! # Why a widget and not a `Style` field
//!
//! Two reasons, and the second decides it (`docs/plan-glass.md` §1.1).
//!
//! `Style` is 72 B with no spare, and a glass parameter set is +20 —
//! paid by every node of every tree, including the embedded profiles
//! that take the `Painter` fallback and cannot render a blur at all. A
//! widget's fields cost nothing where the widget is not used.
//!
//! And a backdrop filter reads **what was painted before it**. A style
//! property is applied wherever the derive happens to emit it, which is
//! not a moment the author chose; a widget occupies a position in the
//! tree, and that position *is* the answer to "what does this blur".
//!
//! # What it does not do
//!
//! **It does not create the overlap.** A `column` allocates
//! non-overlapping rects, so a `Glass` laid out above a scroller has the
//! page behind it and nothing to blur. Put both in a
//! `#[widget(stack)]` (M11/T11.6): the stack's children share its rect,
//! paint in field order, and a `Glass` declared *after* the content
//! blurs it.
//!
//! **`Style::background` is deliberately not painted.** The glass *is*
//! the background, and a fill underneath it would be the thing the blur
//! reads — an opaque backdrop blurs to itself, so the panel would look
//! like a flat rect and the feature would appear broken. The tint is
//! [`Glass::tint`]; `tests/glass_widget.rs` pins that the background is
//! ignored rather than silently composited.

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::display::{Color, Painter};
use crate::hit::Hit;
use crate::ops::{inner_limits, rect_of, resolve_self};
use crate::style::{Length, Style};
use crate::widget::{Node, Widget};
use crate::LayoutCx;

/// Content on a panel that blurs the frame beneath it.
pub struct Glass<W: Widget> {
    pub content: W,
    /// Gaussian standard deviation, in logical px. 0 is a plain tint.
    pub blur: f32,
    /// Multiplied over the blurred backdrop; `tint.3` is its strength.
    /// Alpha 0 is a real value — a pure frosted panel.
    pub tint: Color,
}

impl<W: Widget + Default> Default for Glass<W> {
    fn default() -> Self {
        Glass::new(W::default())
    }
}

impl<W: Widget> Glass<W> {
    /// Content index, relative to this node.
    const AT: usize = 1;

    /// A frosted panel with the default material.
    pub fn new(content: W) -> Self {
        Glass { content, blur: Self::BLUR, tint: Self::TINT }
    }

    /// The default blur, in logical px.
    ///
    /// Chosen to read as frosted rather than as a smear at the size a
    /// toolbar is, and **not measured** — `docs/plan-glass.md` §11 says
    /// nothing in this feature is a measured number until T19.3's sweep.
    pub const BLUR: f32 = 12.0;

    /// The default tint: white at 16%, the strength a light material
    /// uses. Also the value that makes the degraded fallback's alpha
    /// floor visibly a lie, which is why that floor exists.
    pub const TINT: Color = Color(255, 255, 255, 40);

    /// Set the blur, in logical px.
    pub fn blur(mut self, px: f32) -> Self {
        self.blur = px;
        self
    }

    /// Set the tint.
    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }
}

impl<W: Widget> Widget for Glass<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// A panel is exactly as rigid as what is on it.
    ///
    /// Not `true` like [`crate::ScrollView`]'s, whose size comes from
    /// its parent whatever its content does. A glass panel's default
    /// height is `Shrink`, so a taller toolbar makes a taller panel —
    /// which is the relayout boundary's question, and answering `true`
    /// here would let a content change skip a relayout it needed.
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = GlassTable::<W>::SLICE;
}

/// The merged table for one `Glass<W>`, built at compile time — the same
/// technique [`crate::scroll::ScrollTable`] uses, and for the same
/// reason: `SUB_TABLE` is a slice, so its length is a value.
pub struct GlassTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> GlassTable<W> {
    const ARRAY: [NodeDesc; MAX_GLASS_NODES] = {
        assert!(W::NODE_COUNT < MAX_GLASS_NODES, "Glass content exceeds MAX_GLASS_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_GLASS_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        crate::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;
}

/// Upper bound on `1 + W::NODE_COUNT` for one [`Glass`].
pub const MAX_GLASS_NODES: usize = 256;

impl<W: Widget> Node for Glass<W> {
    fn kind_name(&self) -> &'static str {
        "glass"
    }

    /// The container convention: full width, content height. A glass
    /// toolbar spans its stack and is as tall as what is on it.
    fn defaults(&self) -> Style {
        Style { width: Length::FILL, height: Length::Shrink, ..Style::default() }
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
            return crate::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        let inner = inner_limits(limits, me.padding);
        let content = self.content.layout(cx, inner, style, st, i + Self::AT);
        let padded = Size {
            w: content.w + me.padding.horizontal(),
            h: content.h + me.padding.vertical(),
        };
        crate::ops::record_painted(st, i, limits, resolve_self(limits, &me, padded), &me)
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        let me = style[i];
        if !me.visible {
            for k in i..(i + Self::NODE_COUNT).min(st.len()) {
                st[k].pos = at;
            }
            return;
        }
        let to = Point { x: at.x + me.padding.left, y: at.y + me.padding.top };
        self.content.place(to, style, st, i + Self::AT);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        if st[i].shadow > 0.0 {
            out.shadow(r, st[i].radius, st[i].shadow);
        }
        // **The panel, then its content on top.** No `rounded_rect` for
        // `st[i].bg`: see the module note — a fill under the glass is
        // what the blur would read, and an opaque backdrop blurs to
        // itself.
        out.glass(r, st[i].radius, self.blur, self.tint);
        self.content.paint(st, i + Self::AT, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !crate::hit::contains(&st[i], x, y) {
            return None;
        }
        // Content first: a button on the panel is above the panel.
        self.content.hit(st, i + Self::AT, x, y).or(Some(Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        }))
    }

    fn node(&self, i: usize) -> &dyn Node {
        if i == 0 {
            self
        } else {
            self.content.node(i - Self::AT)
        }
    }

    fn node_mut(&mut self, i: usize) -> &mut dyn Node {
        if i == 0 {
            self
        } else {
            self.content.node_mut(i - Self::AT)
        }
    }
}

/// Handlers pass straight through to the content: a panel is a surface,
/// not a control, and node 0 has nothing to answer.
impl<W, S> crate::event::Handlers<S> for Glass<W>
where
    W: Widget + crate::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content.handler(crate::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}
