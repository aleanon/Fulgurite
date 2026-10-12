//! An image widget — `docs/plan-capabilities.md` T15.2.
//!
//! **The core never carries pixels and never decodes.** An
//! [`Image`] holds an [`ImageId`], which the *host* resolves through
//! [`fulgurite::ImageSource`]. That is what keeps images available in the `no_std`
//! profile: an embedded host resolves an id to a `&'static [u8]` in
//! flash, a desktop host to a decoded buffer, and the widget, the
//! command and the layout are identical in both.
//!
//! **Decoding lives in the shell, behind its default-off `image`
//! feature** (M84/T84.1, `docs/results-images.md`):
//! `fulgurite_shell::image::decode_png` and `decode_gif` answer
//! RGBA8 frames, and an `ImageStore` holds them by id as an
//! [`fulgurite::ImageSource`]. PNG and GIF only; JPEG and WebP are not
//! decoded, and an application with other formats decodes itself and
//! fills the store. Until M54/T54.9 this comment named that feature
//! before it existed (R15-F9).

use fulgurite::cx::LayoutCx;
use fulgurite::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use fulgurite::display::{Color, ImageId, Painter};
use fulgurite::hit::Hit;
use fulgurite::ops::{rect_of, resolve_self};
use fulgurite::style::{Seed, Style};
use fulgurite::widget::{Node, Widget};

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
    /// **Which version of the surface to draw** (M84/T84.2,
    /// `docs/results-images.md`): an animated image's frame index, a
    /// stream's counter, 0 for a still. It goes into
    /// [`Cmd::Image`](fulgurite::Cmd::Image), so writing it is what
    /// makes the next frame repaint this image's rect; the source answers
    /// for it ([`fulgurite::ImageSource::image`]). An animated image
    /// is this widget with a `frame` that moves, written by the
    /// application's tick — there is no second widget type for it.
    pub frame: u32,
    /// What the picture shows, for a screen reader — M54/T54.3. Empty is
    /// decoration and publishes no name. Only with `a11y` (a build nobody
    /// reads aloud does not carry it); write it through `set_alt`.
    #[cfg(feature = "a11y")]
    pub alt: &'static str,
}

impl Image {
    pub fn new(id: ImageId, source: Size) -> Self {
        Image {
            id,
            source,
            fit: Fit::default(),
            tint: Color(255, 255, 255, 255),
            frame: 0,
            #[cfg(feature = "a11y")]
            alt: "",
        }
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

    const KIND: &'static str = "image";

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, fulgurite::style::Fields::of(&[
            fulgurite::style::Fields::WIDTH,
            fulgurite::style::Fields::HEIGHT,
            fulgurite::style::Fields::CURSOR,
            fulgurite::style::Fields::VISIBLE,
        ]))];
}

impl Node for Image {
    fulgurite::node_count!();
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
            return fulgurite::ops::hide_subtree(st, i, limits, 1);
        }
        // The intrinsic size is what `Shrink` shrinks to. Without one an
        // image has no opinion and the box decides.
        let size = resolve_self(limits, &me, self.source);
        st[i].size = size;
        st[i].constraints = limits;
        size
    }

    fn place(&mut self, at: fulgurite::desc::Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        fulgurite::ops::fill(st, i, out);
        if st[i].bg.3 > 0 {
            out.rounded_rect(r, st[i].bg, st[i].radius);
            fulgurite::ops::border(st, i, out);
        }
        out.image(self.placed(r), self.id, self.tint, self.frame);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        fulgurite::hit::contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: fulgurite::desc::Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    /// An image, named by [`Image::alt`] — M54/T54.3 (R15-F3).
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut fulgurite::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(fulgurite::observe::accesskit::NodeId, fulgurite::observe::accesskit::Node)>,
    ) {
        use fulgurite::observe::accesskit::Role;
        node.set_role(Role::Image);
        if !self.alt.is_empty() {
            node.set_label(self.alt);
        }
    }
}

impl<S> fulgurite::event::Handlers<S> for Image {
    fn handler(&self, _: fulgurite::event::NodeSel) -> Option<fn(&mut S, &fulgurite::event::EventCtx)> {
        None
    }
}

impl Image {
    /// Describe this image for a screen reader (M54/T54.3). A no-op in a build without `a11y`, which
    /// carries no name.
    #[allow(unused_variables)]
    pub fn set_alt(&mut self, alt: &'static str) {
        #[cfg(feature = "a11y")]
        {
            self.alt = alt;
        }
    }
}
