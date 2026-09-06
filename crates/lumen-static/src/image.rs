//! An image widget — `docs/plan-capabilities.md` T15.2.
//!
//! **The core never carries pixels and never decodes.** An
//! [`Image`] holds an [`ImageId`], which the *host* resolves through
//! [`crate::ImageSource`]. That is what keeps images available in the `no_std`
//! profile: an embedded host resolves an id to a `&'static [u8]` in
//! flash, a desktop host to a decoded buffer, and the widget, the
//! command and the layout are identical in both.
//!
//! Decoding — PNG, JPEG, WebP — is the *shell's* business and lives
//! behind its default-off `image` feature, because that is where the
//! decoder and the allocator are.

use crate::cx::LayoutCx;
use crate::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use crate::display::{Color, ImageId, Painter};
use crate::hit::Hit;
use crate::ops::{rect_of, resolve_self};
use crate::style::Style;
use crate::widget::{Node, Widget};

/// How an image fills a box whose aspect differs from its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    /// Stretch to the box, ignoring aspect. The cheapest, and wrong for
    /// photographs.
    Stretch,
    /// Largest size that fits inside the box, aspect kept. Letterboxed.
    #[default]
    Contain,
    /// Smallest size that covers the box, aspect kept. Cropped.
    Cover,
}

/// A host-owned image, drawn into this node's box.
#[derive(Clone, Copy, Debug, Default)]
pub struct Image {
    pub id: ImageId,
    /// The image's own pixel size, which the *application* supplies —
    /// the core cannot ask, because it never sees the pixels. Used for
    /// [`Fit`] and for `Shrink` layout; zero means "no intrinsic size",
    /// and the box wins.
    pub source: Size,
    pub fit: Fit,
    /// Multiplied into the source. Opaque white is untinted, which is
    /// what lets one monochrome icon serve several colours.
    pub tint: Color,
}

impl Image {
    pub fn new(id: ImageId, source: Size) -> Self {
        Image { id, source, fit: Fit::default(), tint: Color(255, 255, 255, 255) }
    }

    /// The sub-rect of `box_` this image occupies under `fit`.
    fn placed(&self, box_: [f32; 4]) -> [f32; 4] {
        let (bw, bh) = (box_[2] - box_[0], box_[3] - box_[1]);
        if self.fit == Fit::Stretch || self.source.w <= 0.0 || self.source.h <= 0.0 {
            return box_;
        }
        let (sx, sy) = (bw / self.source.w, bh / self.source.h);
        let k = match self.fit {
            Fit::Contain => sx.min(sy),
            Fit::Cover => sx.max(sy),
            Fit::Stretch => 1.0,
        };
        let (w, h) = (self.source.w * k, self.source.h * k);
        // Centred, which is what both fits mean in every toolkit that
        // has them.
        let (ox, oy) = (box_[0] + (bw - w) / 2.0, box_[1] + (bh - h) / 2.0);
        [ox, oy, ox + w, oy + h]
    }
}

impl Widget for Image {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
}

impl Node for Image {
    fn kind_name(&self) -> &'static str {
        "image"
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
            return crate::ops::hide_subtree(st, i, limits, 1);
        }
        // The intrinsic size is what `Shrink` shrinks to. Without one an
        // image has no opinion and the box decides.
        let size = resolve_self(limits, &me, self.source);
        st[i].size = size;
        st[i].constraints = limits;
        size
    }

    fn place(&mut self, at: crate::desc::Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        if st[i].bg.3 > 0 {
            out.rounded_rect(r, st[i].bg, st[i].radius);
        }
        out.image(self.placed(r), self.id, self.tint);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        crate::hit::contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: crate::desc::Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }
}

impl<S> crate::event::Handlers<S> for Image {
    fn handler(&self, _: crate::event::NodeSel) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        None
    }
}
