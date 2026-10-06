//! `VectorText` — a run of text drawn as glyph outlines, so it can turn
//! (M82/T82.8, `docs/plan-paint-vocabulary.md`, "Vector text is outlines
//! from the measurer").
//!
//! A `Label` is drawn from glyph bitmaps, which sit on the pixel grid and
//! cannot rotate. This widget asks the measurer for the run's outlines
//! ([`TextMeasure::outline`](lumen_static::cx::TextMeasure::outline)) and
//! paints them as **one path** whose transform is its rotation
//! ([`Painter::path`]), so the run turns by any angle on either backend.
//!
//! **Its box holds its ink.** The node is sized to the rotated bounds of
//! the unrotated run — the measured line box, grown to the outline's ink
//! where a glyph overhangs it — and the run is turned about its centre in
//! the middle of the node, so the ink stays inside the node's box at every
//! angle (which is why it needs no ink-outside flag). Hit testing stays
//! axis-aligned on that box, as for every node.
//!
//! **Visible beats faithful.** A measurer with no outlines — a baked face,
//! the block stand-in, or `FontMetrics` without the shell's `vector-text`
//! feature — answers `false`, and the widget then paints its string as
//! ordinary axis-aligned text in a box the run's own size: the `Glass`
//! precedent, where the fallback is the plain thing rather than nothing.
//!
//! What it costs, uncached, on the software canvas is K82e's arm of
//! `benches --bin vocabulary` (`docs/results-paint-vocabulary.md` T82.8).

use alloc::vec::Vec;

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use lumen_static::display::{path_bounds, Affine, Color, Painter, PathEl, PathPaint, AFFINE_IDENTITY};
use lumen_static::ops::{border, fill, record_painted, rect_of, resolve_self};
use lumen_static::style::{Fields, Seed, Style};
use lumen_static::text::TextStorage;
use lumen_static::widget::{Node, Widget};

/// A run of text at an angle, drawn as glyph outlines.
///
/// `text` is a plain field, like `Button`'s label: the application or a
/// `#[bind]` writes it, and the memo keys on a hash of its bytes rather
/// than a dirty bit. `angle` is in degrees, **clockwise** (y points down),
/// about the run's centre; 90 reads top to bottom.
///
/// The outline is memoised on (text, size, weight, the measurer's
/// [`generation`](lumen_static::cx::TextMeasure::generation), the display
/// scale): a still frame asks the measurer nothing, and a measurer that
/// changed what it measures — a stack that loaded a face, a new font — is
/// asked again, rather than served the old outline.
pub struct VectorText<S: TextStorage = &'static str> {
    pub text: S,
    /// Clockwise degrees.
    pub angle: f32,
    memo: Option<Outline>,
}

/// What one measurement of the run produced: the key it was taken under,
/// the outline in the run's frame, the run's box (the line box grown to
/// the ink), and whether the measurer had outlines at all.
struct Outline {
    key: Key,
    els: Vec<PathEl>,
    run: [f32; 4],
    drawn: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    text: u64,
    px: u32,
    weight: u32,
    scale: u32,
    generation: u32,
}

impl<S: TextStorage> Default for VectorText<S> {
    fn default() -> Self {
        VectorText { text: S::default(), angle: 0.0, memo: None }
    }
}

impl<S: TextStorage> VectorText<S> {
    pub fn new(text: S, angle: f32) -> Self {
        VectorText { text, angle, memo: None }
    }

    /// Whether the last layout drew outlines (`true`) or fell back to
    /// axis-aligned text — for a test, and for an application that wants
    /// to know which it got.
    pub fn is_vector(&self) -> bool {
        self.memo.as_ref().is_some_and(|m| m.drawn)
    }
}

/// `(sin, cos)` of `deg` degrees — **exact at every quarter turn**, so the
/// box at 90° is the box at 0° with its sides swapped, and allocation- and
/// `std`-free (the widget is in the `alloc` tier, whose `core` has no
/// trigonometry). The remainder within a quarter turn goes through a
/// Taylor series to the 13th power, under 1e-9 at a quarter turn.
pub fn sin_cos_deg(deg: f32) -> (f32, f32) {
    let d = if deg.is_finite() { deg } else { 0.0 };
    let turns = lumen_static::num::floor(d / 360.0);
    let d = d - turns * 360.0;
    let q = (lumen_static::num::floor(d / 90.0) as i32).rem_euclid(4);
    let r = (d - q as f32 * 90.0).clamp(0.0, 90.0);
    let (s, c) = if r == 0.0 {
        (0.0, 1.0)
    } else if r == 90.0 {
        (1.0, 0.0)
    } else {
        let x = r * (core::f32::consts::PI / 180.0);
        let x2 = x * x;
        let s = x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0 * (1.0 - x2 / 156.0))))));
        let c = 1.0 - x2 / 2.0 * (1.0 - x2 / 12.0 * (1.0 - x2 / 30.0 * (1.0 - x2 / 56.0 * (1.0 - x2 / 90.0 * (1.0 - x2 / 132.0)))));
        (s, c)
    };
    match q {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// The width and height of a `w × h` box turned by `(sin, cos)`.
fn turned(w: f32, h: f32, (s, c): (f32, f32)) -> Size {
    Size { w: (w * c).abs() + (h * s).abs(), h: (w * s).abs() + (h * c).abs() }
}

/// Measure `text` at `style`'s size and weight: its outline and run box,
/// or the run's measured box alone when the measurer has no outlines.
fn measure(cx: &mut LayoutCx, text: &str, style: &Style) -> (Vec<PathEl>, [f32; 4], bool) {
    let (px, wt) = (style.font_size, style.font_weight);
    let line = cx.text.measure_weighted(text, px, wt, f32::INFINITY);
    let mut els = Vec::new();
    let drawn = cx.text.outline(text, px, wt, &mut els);
    let mut run = [0.0, 0.0, line.w, line.h];
    if drawn {
        if let Some(b) = path_bounds(&els, &AFFINE_IDENTITY) {
            run = [run[0].min(b[0]), run[1].min(b[1]), run[2].max(b[2]), run[3].max(b[3])];
        }
    }
    (els, run, drawn)
}

impl<S: TextStorage> VectorText<S> {
    /// The content box this run needs: the turned run box when outlined,
    /// the run itself when not.
    fn content(&self, run: [f32; 4], drawn: bool) -> Size {
        let (w, h) = (run[2] - run[0], run[3] - run[1]);
        if drawn {
            turned(w, h, sin_cos_deg(self.angle))
        } else {
            Size { w, h }
        }
    }

    /// The transform that turns the run about its centre and puts that
    /// centre at `(cx, cy)`.
    fn xf(&self, run: [f32; 4], cx: f32, cy: f32) -> Affine {
        let (s, c) = sin_cos_deg(self.angle);
        let (rx, ry) = ((run[0] + run[2]) / 2.0, (run[1] + run[3]) / 2.0);
        [c, s, -s, c, cx - (c * rx - s * ry), cy - (s * rx + c * ry)]
    }
}

impl<S: TextStorage> Widget for VectorText<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "vectortext";

    /// A display size: a run turned on its side is a heading, not body
    /// text, and at 16 px its rotation would be the least of its problems.
    fn type_defaults() -> Style {
        Style { font_size: 24.0, ..Style::default() }
    }

    const SEEDS: &'static [Seed] = &[Seed::new(Self::KIND, Self::type_defaults, Fields::of(&[
        Fields::WIDTH,
        Fields::HEIGHT,
        Fields::PADDING,
        Fields::CURSOR,
        Fields::VISIBLE,
        Fields::COLOR,
        Fields::BACKGROUND,
        Fields::BORDER,
        Fields::BACKGROUND_FILL,
        Fields::RADIUS,
        Fields::FONT_SIZE,
        Fields::FONT_WEIGHT,
        Fields::PRESSED_DIM,
        Fields::HOVER_DIM,
        Fields::TRANSITION,
    ]))
    .with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];
}

impl<S: TextStorage> Node for VectorText<S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = &style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let text = self.text.as_ref();
        let key = Key {
            text: lumen_static::display::hash_bytes(text.as_bytes()),
            px: me.font_size.to_bits(),
            weight: me.font_weight.to_bits(),
            scale: cx.scale().to_bits(),
            generation: cx.generation(),
        };
        if self.memo.as_ref().is_none_or(|m| m.key != key) {
            let (els, run, drawn) = measure(cx, text, me);
            self.memo = Some(Outline { key, els, run, drawn });
        }
        let m = self.memo.as_ref().expect("measured above");
        let content = self.content(m.run, m.drawn);
        let pad = me.padding;
        let boxed = Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, me, boxed), me)
    }

    /// Measure without touching the memo — see [`Node::probe`]: the
    /// agent's audit measures with its own measurer, whose generation
    /// would otherwise throw the frame's outline away twice a frame.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = &style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let (_, run, drawn) = measure(cx, self.text.as_ref(), me);
        let content = self.content(run, drawn);
        let pad = me.padding;
        let boxed = Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, me, boxed), me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        fill(st, i, out);
        out.rounded_rect(r, s.bg, s.radius);
        border(st, i, out);
        let Some(m) = &self.memo else { return };
        let p = s.pad;
        let content = [r[0] + p.0, r[1] + p.1, r[2] - p.2, r[3] - p.3];
        if !m.drawn {
            // The fallback: the string, axis-aligned, in a box its own size.
            out.text_spaced(content, self.text.as_ref(), s.font_size, s.font_weight, 0, s.fg);
            return;
        }
        let (cx, cy) = ((content[0] + content[2]) / 2.0, (content[1] + content[3]) / 2.0);
        let none = Color(0, 0, 0, 0);
        out.path(&m.els, self.xf(m.run, cx, cy), PathPaint { fill: s.fg, stroke: none, width: 0.0 });
    }

    lumen_static::leaf_common!(VectorText<S>, 0);

    /// A label: the run is text a reader should hear, whatever its angle.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Label);
        node.set_value(self.text.as_ref());
    }
}

impl<A, S: TextStorage> lumen_static::event::Handlers<A> for VectorText<S> {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut A, &lumen_static::event::EventCtx)> {
        None
    }
}
