//! A node a custom shader draws — M83/T83.6 (`docs/plan-gpu-extension.md`),
//! behind this crate's `shader` feature, which turns the core's on.
//!
//! **A `ShaderView` paints a bracket, never pixels of its own.** Its
//! `paint` is [`Painter::shader`] over its box with its uniform bytes, then
//! its [`ShaderView::fallback`] drawn with ordinary `Painter` calls, then
//! [`Painter::shader_end`]. A backend that runs the program's shader `id`
//! draws the pipeline over the rect and skips the fallback; one that cannot
//! — the software canvas, SVG, a GPU that does not know the id, refused the
//! WGSL or was handed the wrong uniform length — draws the fallback clipped
//! to the rect. So `ui.screenshot`, `LUMEN_SHOT` and every headless host
//! show the fallback, and the journal and retention see the same list on
//! every backend (the `Cmd::Shader` rustdoc in the core).
//!
//! **The id and the bytes live here, not in `Style` or `NodeState`**, so no
//! per-node table grows for a widget most trees never name
//! (`docs/results-shader.md` T83.6 prints `size_of::<ShaderView<16>>()`).
//! The uniforms are content-hashed by the list and never by offset, so a
//! view whose uniforms grew does not report the views painted after it
//! (M83/T83.3; `lumen-static-agent/tests/damage_shape.rs` drives that end
//! to end through the journal).
//!
//! **A write marks the node as any write does.** [`ShaderView::set_uniforms`]
//! and [`ShaderView::set_shader`] compare before they store and say whether
//! anything changed, and a `#[bind(ui.view, state.bytes)]` binding — the
//! value form, through this type's [`BindValue`] impl — marks the node from
//! that answer. A setter that stored without saying so would leave the
//! frame skipped under `Boundaries` and the shader drawing last frame's
//! bytes.

use lumen_static::bind::BindValue;
use lumen_static::cx::LayoutCx;
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use lumen_static::display::{Color, Painter, Rect, ShaderId, MAX_UNIFORMS};
use lumen_static::hit::Hit;
use lumen_static::ops::{rect_of, resolve_self};
use lumen_static::style::{Seed, Style};
use lumen_static::widget::{Node, Widget};

/// What a [`ShaderView`] draws where its shader does not run: handed the
/// painter, the view's box, and the uniform bytes the shader would have
/// received. A plain `fn`, never a closure — the handler rule
/// (`docs/events-decision.md`) — so it is re-supplied by a hot library
/// like any other code, and it runs at paint time, in the current build.
pub type Fallback = fn(&mut dyn Painter, Rect, &[u8]);

/// A box drawn by the program's custom shader `id`, with `N` bytes of
/// uniforms (`N` ≤ [`MAX_UNIFORMS`], checked at compile time) and a
/// fallback for every backend that does not run it.
///
/// The shader is registered on the program (`Program::shaders`, a table of
/// `ShaderDesc`s in `lumen-static-shell`); `id` is its index there, fixed
/// for a build. The bytes reach the WGSL as its `@group(0) @binding(1)`
/// block, and the GPU draws the shader only when their length equals the
/// entry's `uniform_size` — so a view switching to a shader with a larger
/// block sends a longer prefix of `uniforms` ([`ShaderView::set_shader`]).
///
/// It has no size of its own unless one is given: [`ShaderView::size`] is
/// what `Shrink` shrinks to, and zero means the box decides, as an
/// [`Image`](crate::Image) without a source does.
#[derive(Clone, Copy, Debug)]
pub struct ShaderView<const N: usize> {
    /// The shader, by its index in the program's table.
    pub id: ShaderId,
    /// The uniform bytes; the first [`ShaderView::len`] of them are sent.
    /// Written through [`ShaderView::set_uniforms`], which says whether
    /// they changed.
    uniforms: [u8; N],
    /// How many of `uniforms` the bracket carries: `N` unless a
    /// [`ShaderView::set_shader`] or a shorter
    /// [`ShaderView::set_uniforms`] said otherwise.
    len: u16,
    /// Drawn inside the bracket, clipped to the box, wherever the shader
    /// does not run. [`ShaderView::solid`] by default.
    pub fallback: Fallback,
    /// The intrinsic size `Shrink` resolves to; zero on an axis means the
    /// box decides it.
    pub size: Size,
    /// What the picture shows, for a screen reader — the author's label.
    /// Empty is decoration and publishes no name. Only with `a11y`, as
    /// [`Image`](crate::Image)'s `alt`; written through
    /// [`ShaderView::set_label`].
    #[cfg(feature = "a11y")]
    pub label: &'static str,
}

impl<const N: usize> ShaderView<N> {
    /// `N` fits one bracket: the list records at most [`MAX_UNIFORMS`]
    /// bytes, and the GPU's slot is that size. Evaluated by every
    /// constructor, so a `ShaderView<300>` is a build error, not a
    /// truncated block.
    const FITS: () = assert!(N <= MAX_UNIFORMS, "a ShaderView carries at most MAX_UNIFORMS (256) uniform bytes");

    /// Shader `id`, all `N` uniform bytes zero, drawn by `fallback` where
    /// the shader does not run.
    pub fn new(id: ShaderId, fallback: Fallback) -> Self {
        #[allow(clippy::let_unit_value)]
        let () = Self::FITS;
        ShaderView {
            id,
            uniforms: [0; N],
            len: N as u16,
            fallback,
            size: Size { w: 0.0, h: 0.0 },
            #[cfg(feature = "a11y")]
            label: "",
        }
    }

    /// The same view with an intrinsic size.
    pub fn sized(mut self, w: f32, h: f32) -> Self {
        self.size = Size { w, h };
        self
    }

    /// The bytes the bracket carries: the first [`ShaderView::len`] of the
    /// uniforms.
    pub fn uniforms(&self) -> &[u8] {
        &self.uniforms[..self.len as usize]
    }

    /// How many uniform bytes the bracket carries.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether the bracket carries no uniform bytes (a shader with no
    /// `@binding(1)` block, `uniform_size` 0).
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Store `bytes` as the uniforms the bracket carries, and say whether
    /// that changed anything. `bytes.len()` becomes [`ShaderView::len`];
    /// more than `N` is a programming error and panics.
    ///
    /// **Compares first**, as every bind target does: identical bytes
    /// answer `false`, so a binding that runs every frame marks nothing
    /// on a frame that changed nothing, and a `true` is what marks the
    /// node (`lumen_static::bind`, "Change detection compares the
    /// output").
    pub fn set_uniforms(&mut self, bytes: &[u8]) -> bool {
        assert!(bytes.len() <= N, "{} uniform bytes into a ShaderView<{N}>", bytes.len());
        if self.uniforms() == bytes {
            return false;
        }
        self.uniforms[..bytes.len()].copy_from_slice(bytes);
        self.len = bytes.len() as u16;
        true
    }

    /// Switch to shader `id`, whose uniform block is `len` bytes (its
    /// `ShaderDesc`'s `uniform_size`), and say whether that changed
    /// anything. The bytes are kept; `len` past `N` panics.
    pub fn set_shader(&mut self, id: ShaderId, len: usize) -> bool {
        assert!(len <= N, "a {len}-byte uniform block in a ShaderView<{N}>");
        let changed = self.id != id || self.len as usize != len;
        self.id = id;
        self.len = len as u16;
        changed
    }

    /// The default [`Fallback`]: the box filled with the colour in the
    /// first four uniform bytes (R, G, B, A), or nothing when there are
    /// fewer. It is the fallback a shader that writes one uniform colour
    /// is pixel-identical to, which is what `lumen-static-wgpu`'s parity
    /// region compares.
    pub fn solid(out: &mut dyn Painter, r: Rect, u: &[u8]) {
        if let [r0, g, b, a, ..] = *u {
            out.rect(r, Color(r0, g, b, a));
        }
    }

    /// Describe what the shader shows, for a screen reader. A no-op in a
    /// build without `a11y`, which carries no name.
    #[allow(unused_variables)]
    pub fn set_label(&mut self, label: &'static str) {
        #[cfg(feature = "a11y")]
        {
            self.label = label;
        }
    }
}

impl<const N: usize> Default for ShaderView<N> {
    /// Shader 0, zero bytes, the [`ShaderView::solid`] fallback (which
    /// draws nothing over a transparent-black block).
    fn default() -> Self {
        ShaderView::new(ShaderId(0), ShaderView::<N>::solid)
    }
}

/// The value form of `#[bind]` — `#[bind(ui.view, bytes)]` with a
/// `[u8; N]` source — writes through [`ShaderView::set_uniforms`], whose
/// answer is what marks the node.
impl<const N: usize> BindValue<[u8; N]> for ShaderView<N> {
    fn bind_value(&mut self, value: &[u8; N]) -> bool {
        self.set_uniforms(value)
    }
}

impl<const N: usize> Widget for ShaderView<N> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

    const KIND: &'static str = "shaderview";

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::VISIBLE,
        ]))];
}

impl<const N: usize> Node for ShaderView<N> {
    lumen_static::node_count!();
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let size = resolve_self(limits, &me, self.size);
        st[i].size = size;
        st[i].constraints = limits;
        size
    }

    fn place(&mut self, at: lumen_static::desc::Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    // ANCHOR: bracket
    /// The bracket: the shader over the box with its bytes, the fallback
    /// inside it, then the end. Nothing else is painted inside, so what a
    /// backend that cannot run the shader shows is exactly the fallback.
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        out.shader(r, self.id, self.uniforms());
        (self.fallback)(out, r, self.uniforms());
        out.shader_end();
    }
    // ANCHOR_END: bracket

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        lumen_static::hit::contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: lumen_static::desc::Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    /// A picture, named by the author's [`ShaderView::label`].
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Image);
        if !self.label.is_empty() {
            node.set_label(self.label);
        }
    }
}

impl<S, const N: usize> lumen_static::event::Handlers<S> for ShaderView<N> {
    fn handler(&self, _: lumen_static::event::NodeSel) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        None
    }
}
