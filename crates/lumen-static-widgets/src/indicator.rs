//! Progress, and the two ways of saying "not yet": a spinner and a
//! skeleton.
//!
//! All three are leaves, and two of them animate. Neither animates
//! *itself*: there is no framework-walked per-node tick — `App::tick`
//! records why — so the application advances the phase and binds it,
//! exactly as it does for `Glass`'s press. A widget that drove its own
//! clock would need a virtual call per node per frame on trees that
//! animate nothing.

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::ops::{record_painted, rect_of, resolve_self};
use lumen_static::style::{Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, Point, Size};

const LEAF: &[NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

/// A determinate bar.
///
/// `value` is a presentation copy in `0..=1`, clamped on the way to the
/// screen rather than on the way in: a binding that produced 1.4 should
/// draw a full bar, not panic, and clamping at the edge keeps the
/// application's own number untouched.
#[derive(Default)]
pub struct ProgressBar {
    /// How full, `0.0..=1.0`. Bind it.
    pub value: f32,
}

impl ProgressBar {
    pub fn new(value: f32) -> Self {
        ProgressBar { value }
    }
    /// The drawn fraction: clamped, and **NaN lands at zero**. Spelled
    /// with the comparison this way round because every comparison
    /// against NaN is false, so `v > 0.0` is the arm that rejects it —
    /// the same trick `Animated::settled` uses.
    fn fraction(&self) -> f32 {
        if self.value > 0.0 {
            self.value.min(1.0)
        } else {
            0.0
        }
    }
}

impl Widget for ProgressBar {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = "progressbar";

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(BAR_H),
            radius: BAR_H / 2.0,
            background: Color(0xe2, 0xe8, 0xf0, 0xff),
            color: Color(0x2f, 0x6b, 0xff, 0xff),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
        ]))];
}

const BAR_H: f32 = 8.0;

impl Node for ProgressBar {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: 0.0, h: BAR_H });
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let radius = s.radius.min(h / 2.0);
        out.rounded_rect(r, s.bg, radius);
        lumen_static::ops::border(st, i, out);
        let filled = w * self.fraction();
        // **Not painted at all when empty.** A zero-width rounded rect
        // with a radius still covers `2 × radius` of pixels at the
        // renderer's clamp, so a 0% bar would show a dot.
        if filled > 0.5 {
            out.rounded_rect([r[0], r[1], r[0] + filled.max(radius * 2.0), r[3]], s.fg, radius);
        }
    }

    /// How full, as a number — M48/T48.6.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::ProgressIndicator);
        node.set_numeric_value(self.fraction() as f64);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
    }

    lumen_static::leaf_common!(ProgressBar, 0, nohit);
}

/// An indeterminate spinner: a ring with a travelling gap.
///
/// `phase` is turns, `0.0..=1.0`, and **the application advances it**.
/// A spinner that owned its own clock would be the per-node tick the
/// framework does not have.
#[derive(Default)]
pub struct Spinner {
    /// Where the gap is, in turns. Bind it from `App::tick`.
    pub phase: f32,
}

impl Spinner {
    pub fn new(phase: f32) -> Self {
        Spinner { phase }
    }
}

impl Widget for Spinner {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = "spinner";

    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(SPIN),
            height: Length::Fixed(SPIN),
            color: Color(0x2f, 0x6b, 0xff, 0xff),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
        ]))];
}

const SPIN: f32 = 28.0;
/// Segments in the ring. Twelve reads as a circle at these sizes and is
/// twelve commands; the renderer has no arc.
impl Node for Spinner {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: SPIN, h: SPIN });
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        // **A ring with a gap, not a sunburst.** The first version drew
        // twelve *radial* spokes fading around the circle, which is the
        // Windows-95 hourglass look; every toolkit's spinner — Lumen's
        // included — is a stroked circle with a arc missing, turning.
        // The difference is the direction of the segments: tangential,
        // not radial.
        let d = w.min(h);
        let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        let radius = d * 0.40;
        let stroke = (d * 0.10).max(1.5);
        let steps = crate::chart::STEPS_ROUND;
        // How much of the circle is missing. A quarter reads as a
        // spinner; much less and it is a circle, much more and it is a
        // comma.
        let gap = steps / 4;
        // `rem_euclid`, so a clock that went backwards still lands
        // inside the ring rather than lighting nothing.
        let lead = (lumen_static::num::fract_euclid(self.phase) * steps as f32) as usize;
        for k in 0..steps - gap {
            let a = crate::chart::unit((lead + k) % steps);
            let b = crate::chart::unit((lead + k + 1) % steps);
            out.line(
                Point { x: cx + a.0 * radius, y: cy + a.1 * radius },
                Point { x: cx + b.0 * radius, y: cy + b.1 * radius },
                stroke,
                s.fg,
            );
        }
    }

    /// An indeterminate progress indicator, marked busy — M48/T48.6.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::ProgressIndicator);
        node.set_busy();
    }

    lumen_static::leaf_common!(Spinner, 0, nohit);
}

/// A placeholder block for content that has not arrived.
///
/// The shimmer is a `phase` the application advances, like [`Spinner`]'s.
/// At `phase == 0` it is a flat block, which is what a host that
/// animates nothing gets and is still the right picture.
#[derive(Default)]
pub struct Skeleton {
    /// Where the highlight is, in `0.0..=1.0` of the width.
    pub phase: f32,
}

impl Skeleton {
    pub fn new(phase: f32) -> Self {
        Skeleton { phase }
    }
}

impl Widget for Skeleton {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = LEAF;

    const KIND: &'static str = "skeleton";

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(16.0),
            radius: 4.0,
            background: Color(0xe4, 0xe9, 0xf2, 0xff),
            color: Color(0xf4, 0xf7, 0xfc, 0xff),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
        ]))];
}

impl Node for Skeleton {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        let size = resolve_self(limits, &me, Size { w: 0.0, h: 16.0 });
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius.min(h / 2.0));
        lumen_static::ops::border(st, i, out);
        // The highlight, clipped to the block so a band near either end
        // does not paint past it. `push_clip`/`pop_clip` rather than
        // arithmetic, because the renderer's clip is a hard test and
        // the arithmetic would have to repeat the rounded corner.
        let band = w * 0.28;
        let x = r[0] - band + (w + band * 2.0) * lumen_static::num::fract_euclid(self.phase);
        out.push_clip(r);
        out.rounded_rect([x, r[1], x + band, r[3]], s.fg, s.radius.min(h / 2.0));
        out.pop_clip();
    }

    // **No `observe`, on purpose** (M48/T48.6): a loading placeholder is
    // decoration; the busy state belongs to what replaces it. It stays
    // `GenericContainer`, allowed by name in the agent's coverage walk.
    lumen_static::leaf_common!(Skeleton, 0, nohit);
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S> lumen_static::event::Handlers<S> for ProgressBar {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S> lumen_static::event::Handlers<S> for Spinner {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

/// **No handlers.** Every widget in a tree must answer `Handlers`, and a
/// widget that carries no `on_*` field answers `None` — dispatch then
/// walks past it. This is one impl per type rather than a blanket one
/// because a blanket `impl<W, S> Handlers<S> for W` would collide with
/// every widget that *does* have handlers.
impl<S> lumen_static::event::Handlers<S> for Skeleton {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}
