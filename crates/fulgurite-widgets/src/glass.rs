//! A panel that blurs what is behind it — M19/T19.1.
//!
//! `Glass<W>` is one node with one child. It emits [`Cmd::Glass`](fulgurite::display::Cmd::Glass) over
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
//! [`Material::tint`]; `tests/glass_widget.rs` pins that the background is
//! ignored rather than silently composited.

use fulgurite::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use fulgurite::display::{Material, Painter};
use fulgurite::hit::Hit;
use fulgurite::ops::{inner_limits, rect_of, resolve_self};
use fulgurite::style::{Length, Seed, Style};
use fulgurite::widget::{Node, Widget};
use fulgurite::LayoutCx;

/// Content on a panel made of glass.
///
/// The material is one field, not a flattened copy of [`Material`]'s.
/// It was flattened until M22/T22.0, when the count reached eight on its
/// way to ten and the round trip was written out twice — and when
/// T22.4's press deformation made a public `material()` accessor a lie,
/// because the material the panel actually paints with depends on state
/// the accessor cannot see. [`Glass::material_now`] is that function.
pub struct Glass<W: Widget> {
    pub content: W,
    pub material: Material,
    /// How far into the press, `0` at rest and `1` held — M22/T22.4.
    ///
    /// Private: it is retargeted from the node array in `layout` and
    /// advanced by [`Glass::tick`], and a caller setting it directly
    /// would be overwritten on the next layout.
    press: fulgurite::anim::Animated<f32>,
}

impl<W: Widget + Default> Default for Glass<W> {
    fn default() -> Self {
        Glass::new(W::default())
    }
}

impl<W: Widget> Glass<W> {
    /// Content index, relative to this node.
    const AT: usize = 1;

    /// A frosted panel: [`Material::FROSTED`].
    pub fn new(content: W) -> Self {
        Glass::with(content, Material::FROSTED)
    }

    /// A see-through panel whose edge bends and catches the light:
    /// [`Material::LIQUID`].
    pub fn liquid(content: W) -> Self {
        Glass::with(content, Material::LIQUID)
    }

    /// A panel with an explicit material.
    pub fn with(content: W, material: Material) -> Self {
        let mut press = fulgurite::anim::Animated::new(0.0);
        // Faster than the 200 ms default: a press is a response to a
        // finger, and anything slower reads as lag rather than as
        // material.
        press.duration = 110.0;
        Glass { content, material, press }
    }

    /// How much the rim thickens at a full press — M22/T22.4.
    ///
    /// Chosen rather than measured: it is the largest deformation that
    /// still reads as the same panel. The refraction deepens further,
    /// because a squashed edge bends light more than it grows.
    const PRESS_RIM: f32 = 0.55;
    /// How much the refraction deepens at a full press.
    const PRESS_REFRACT: f32 = 0.85;

    /// The material this panel paints with **right now**.
    ///
    /// Not a plain accessor: a pressed panel squashes, so the material
    /// the renderer receives is the declared one modulated by an
    /// animated value. Nothing here is a new [`Material`] field and the
    /// renderer is unchanged — the widget assembles what it asks for.
    pub fn material_now(&self) -> Material {
        let p = self.press.get();
        if p <= 0.0 {
            // **Exactly the declared material at rest**, not a multiply
            // by 1.0: the settled frame has to be byte-identical to the
            // pre-press one, and a float round trip is not free to
            // assume.
            return self.material;
        }
        Material {
            rim: self.material.rim * (1.0 + Self::PRESS_RIM * p),
            refraction: self.material.refraction * (1.0 + Self::PRESS_REFRACT * p),
            ..self.material
        }
    }

    /// Advance the press animation by `dt` milliseconds; returns whether
    /// it moved — M22/T22.4.
    ///
    /// **The application calls this from [`fulgurite::App::tick`].** There is
    /// no framework-walked `Node::tick`, and that is a recorded decision
    /// rather than an omission: it would cost a virtual call per node
    /// per frame on trees that animate nothing — 3 000 of them on the
    /// benchmark's list — which the M15 gating policy forbids. See
    /// [`fulgurite::App::tick`].
    ///
    /// It advances **this** panel only. A `Glass` nested inside another
    /// panel's content is a separate animation and needs its own call;
    /// recursing would mean a `tick` on `Widget`, which is the per-node
    /// pass this design rejects.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        self.press.tick(dt_ms)
    }
}

impl<W: Widget> Widget for Glass<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// A panel is exactly as rigid as what is on it.
    ///
    /// Not `true` like [`ScrollView`](crate::scroll::ScrollView)'s, whose size comes from
    /// its parent whatever its content does. A glass panel's default
    /// height is `Shrink`, so a taller toolbar makes a taller panel —
    /// which is the relayout boundary's question, and answering `true`
    /// here would let a content change skip a relayout it needed.
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = GlassTable::<W>::SLICE;

    const KIND: &'static str = "glass";

    const SEEDS: &'static [Seed] = GlassTable::<W>::SEED_SLICE;

    /// The container convention: full width, content height. A glass
    /// toolbar spans its stack and is as tall as what is on it.
    fn type_defaults() -> Style {
        Style { width: Length::FILL, height: Length::Shrink, ..Style::default() }
    }
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
        fulgurite::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    /// The seed counterpart of [`Self::ARRAY`], spliced the same way.
    const SEED_ARRAY: [Seed; MAX_GLASS_NODES] = {
        let mut s = [Seed::WIDGET; MAX_GLASS_NODES];
        s[0] = Seed::new(<Glass<W> as Widget>::KIND, <Glass<W> as Widget>::type_defaults, fulgurite::style::Fields::of(&[
                fulgurite::style::Fields::WIDTH,
                fulgurite::style::Fields::HEIGHT,
                fulgurite::style::Fields::PADDING,
                fulgurite::style::Fields::CURSOR,
                fulgurite::style::Fields::VISIBLE,
                fulgurite::style::Fields::RADIUS,
                fulgurite::style::Fields::SHADOW,
            ]),);
        fulgurite::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

/// Upper bound on `1 + W::NODE_COUNT` for one [`Glass`].
pub const MAX_GLASS_NODES: usize = 256;

impl<W: Widget> Node for Glass<W> {
    fulgurite::node_count!();
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
            return fulgurite::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        // **Retarget here — M22/T22.4.** `layout` is the one method
        // holding both `&mut self` and the state array, and `dispatch`
        // has already run this pump, so the flag is this frame's.
        //
        // **The whole subtree, not this node.** `dispatch` marks the
        // deepest hit node only, so for `Glass<Button>` the *button*
        // carries the flag and the panel never does. A test that
        // asserted "the flag changed" on the panel would compare a
        // frame with itself forever.
        let hi = (i + Self::NODE_COUNT).min(st.len());
        let down = st[i..hi].iter().any(|n| n.pressed);
        self.press.go(if down { 1.0 } else { 0.0 });
        // Inside its own `Fixed` size, not across the offer (M57/B26:
        // the same defect M56 found in the derive, in a hand-written
        // wrapper; `tests/wrapper_own_size.rs`).
        let inner = inner_limits(fulgurite::ops::within_own_size(limits, &me), me.padding);
        let content = self.content.layout(cx, inner, style, st, i + Self::AT);
        let padded =
            Size { w: content.w + me.padding.horizontal(), h: content.h + me.padding.vertical() };
        fulgurite::ops::record_painted(st, i, limits, resolve_self(limits, &me, padded), &me)
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
        // **The union of both blobs — M22/T22.5.** The command's rect
        // is the ink, the clip test and the source, so a second blob
        // outside it would be stale, holed and short of backdrop. The
        // renderer derives the blobs back out of the union.
        let m = self.material_now();
        out.glass(fulgurite::display::glass_union(r, &m), st[i].radius, m);
        self.content.paint(st, i + Self::AT, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !fulgurite::hit::contains(&st[i], x, y) {
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
impl<W, S> fulgurite::event::Handlers<S> for Glass<W>
where
    W: Widget + fulgurite::event::Handlers<S>,
{
    fn handler(&self, sel: fulgurite::event::NodeSel) -> Option<fn(&mut S, &fulgurite::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content.handler(fulgurite::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}
