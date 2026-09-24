//! **The live readout, as a chip that is absent when it has nothing to
//! say** — M56/T56.3 (`docs/plan-showcase-design.md`).
//!
//! A `Label` with a background and padding paints its pill whatever its
//! text, so a demo that carries no state — most of them — showed an
//! empty 20 × 8 px lozenge beside its title. A style cannot hide a node
//! per frame, so the chip does it itself: with no text it lays out at
//! 0 × 0 and paints nothing. It publishes its text to the agent exactly as
//! a `Label` does (role `Label`, text as the value), because the drive
//! scripts read `#readout`.

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{EventCtx, Handlers, NodeSel};
use lumen_static::ops::{rect_of, record_painted, resolve_self};
use lumen_static::style::{Edges, Fields, Seed, Style};
use lumen_static::text::{FoldedText, MeasureKey};
use lumen_static::widget::{Node, Widget};

#[derive(Default)]
pub struct StatusChip {
    text: String,
    measured: FoldedText,
}

impl StatusChip {
    /// Write `text`; whether it differed — so the caller can report a
    /// change honestly and an unchanged frame allocates nothing.
    pub fn set_str(&mut self, text: &str) -> bool {
        if self.text == text {
            return false;
        }
        self.text.clear();
        self.text.push_str(text);
        true
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl Widget for StatusChip {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "statuschip";

    fn type_defaults() -> Style {
        Style {
            font_size: 12.0,
            font_weight: 600.0,
            radius: 12.0,
            padding: Edges { left: 10.0, right: 10.0, top: 4.0, bottom: 4.0 },
            color: Color(47, 107, 255, 255),
            background: Color(232, 239, 255, 255),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] = &[Seed::new(
        Self::KIND,
        Self::type_defaults,
        Fields::of(&[
            Fields::VISIBLE,
            Fields::PADDING,
            Fields::COLOR,
            Fields::BACKGROUND,
            Fields::RADIUS,
            Fields::FONT_SIZE,
            Fields::FONT_WEIGHT,
        ]),
    )];
}

impl Node for StatusChip {
    fn layout(&mut self, cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        if self.text.is_empty() {
            // **Absent, not merely unpainted**: no box means no pill, and
            // nothing for a hit test to land on.
            return record_painted(st, i, limits, Size::default(), &style[i]);
        }
        let (px, weight) = (style[i].font_size, style[i].font_weight);
        let key = MeasureKey::of(limits, &style[i], cx.scale());
        let text = &self.text;
        let m = self.measured.get_or(key, text, || cx.text.measure_weighted(text, px, weight, limits.max_w));
        let pad = style[i].padding;
        let content = Size { w: m.w + pad.horizontal(), h: m.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        if self.text.is_empty() || s.size.w <= 0.0 {
            return;
        }
        let r = rect_of(s);
        out.rounded_rect(r, s.bg, s.radius.min((r[3] - r[1]) / 2.0));
        let m = self.measured.size();
        let x = r[0] + ((r[2] - r[0]) - m.w).max(0.0) / 2.0;
        let y = r[1] + ((r[3] - r[1]) - m.h).max(0.0) / 2.0;
        out.text_weighted([x, y, x + m.w, y + m.h], &self.text, s.font_size, s.font_weight, s.fg);
    }

    #[cfg(feature = "agent")]
    fn probe(&mut self, cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        if self.text.is_empty() {
            return record_painted(st, i, limits, Size::default(), &style[i]);
        }
        let m = cx.text.measure_weighted(&self.text, style[i].font_size, style[i].font_weight, limits.max_w);
        let pad = style[i].padding;
        let content = Size { w: m.w + pad.horizontal(), h: m.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }

    lumen_static::leaf_common!(StatusChip, 0, nohit);

    #[cfg(feature = "agent")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Label);
        node.set_value(self.text.as_str());
    }
}

impl<S> Handlers<S> for StatusChip {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}
