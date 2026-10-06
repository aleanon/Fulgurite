//! **The sidebar's row** — M56/T56.2 (`docs/plan-showcase-design.md`).
//!
//! One row type for both a group heading and an entry, because the
//! sidebar is one `Repeat` and a `Repeat` holds one row type. What a row
//! is — its text, heading or entry, selected or not — is written by the
//! repeater's bind, which is a plain `fn(&mut NavRow, usize)` and cannot
//! see the application's state. So **the selection is a thread-local**
//! ([`set_selected`]) that `rebuild` writes and the bind reads — a
//! thread-local and not a process-wide static because the tests run one
//! `Show` per thread, each with its own selection. That is the honest cost
//! of a bind that cannot capture, and it is also why a selection change
//! needs a whole-tree mark: `refresh` reports it, and showcase's
//! `after_update` is on M29 §2.4's conservative path, so the repeater
//! re-binds on that frame.
//!
//! **The selected row's colours come from the stylesheet, not from
//! here**, so a palette swap re-themes it: its text is the style's `color`,
//! its base is the style's `background` (which `hover_dim` darkens), and
//! its accent — the tint behind it and the bar at its edge — is the
//! style's `border_color`. The border itself is never stroked: a `Repeat`
//! row has no per-row style, and `Style` has no accent slot of its own.

use std::cell::Cell;

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{EventCtx, Handlers, NodeSel};
use lumen_static::ops::{record_painted, rect_of, resolve_self};
use lumen_static::style::{Fields, Seed, Style};
use lumen_static::text::{FoldedText, MeasureKey};
use lumen_static::widget::{Node, Widget};

std::thread_local! {
    /// Which catalogue entry the sidebar highlights — see the module doc
    /// for why it is not a field.
    static SELECTED: Cell<usize> = const { Cell::new(0) };
}

/// The entry the sidebar highlights on this thread.
pub fn selected() -> usize {
    SELECTED.with(Cell::get)
}

/// Written by `rebuild`, whenever the selection changes.
pub fn set_selected(i: usize) {
    SELECTED.with(|c| c.set(i));
}

/// A heading names a group and does nothing when pressed; an item picks
/// its entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowKind {
    #[default]
    Item,
    Heading,
}

/// A heading's text alpha over the row's text colour: the muted role,
/// derived, so it follows a palette swap. `chrome_text_meets_contrast`
/// checks the result.
pub const HEADING_ALPHA: u8 = 0xa8;
/// The selected row's accent tint alpha, over the row's ground.
pub const SELECTED_TINT_ALPHA: u8 = 0x24;

/// Horizontal inset of a row's pill inside the sidebar.
const INSET: f32 = 8.0;
/// Where the text starts inside the pill.
const TEXT_X: f32 = 12.0;

pub struct NavRow<S: 'static> {
    pub label: &'static str,
    pub kind: RowKind,
    pub selected: bool,
    pub on_click: fn(&mut S, &EventCtx),
    measured: FoldedText,
}

fn ignore<S>(_: &mut S, _: &EventCtx) {}

impl<S> Default for NavRow<S> {
    fn default() -> Self {
        NavRow { label: "", kind: RowKind::Item, selected: false, on_click: ignore::<S>, measured: FoldedText::new() }
    }
}

impl<S: 'static> Widget for NavRow<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "navrow";

    fn type_defaults() -> Style {
        Style {
            radius: 8.0,
            font_size: 14.0,
            hover_dim: 0.05,
            pressed_dim: 0.1,
            cursor: lumen_static::style::Cursor::Pointer,
            color: Color(27, 34, 48, 255),
            border_color: Color(47, 107, 255, 255),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] = &[Seed::new(
        Self::KIND,
        Self::type_defaults,
        Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::CURSOR,
            Fields::VISIBLE,
            Fields::COLOR,
            Fields::BACKGROUND,
            Fields::BORDER,
            Fields::RADIUS,
            Fields::FONT_SIZE,
            Fields::HOVER_DIM,
            Fields::PRESSED_DIM,
            Fields::TRANSITION,
        ]),
    )];
}

impl<S> NavRow<S> {
    /// A heading is smaller and bolder than an item, and drawn in the
    /// text colour at reduced alpha — the muted role, derived, so it
    /// follows a palette swap with the rest of the row.
    fn face(&self, st: &NodeState) -> (f32, f32, Color) {
        match self.kind {
            RowKind::Heading => {
                let c = st.fg;
                (st.font_size - 2.0, 700.0, Color(c.0, c.1, c.2, HEADING_ALPHA))
            }
            RowKind::Item if self.selected => (st.font_size, 600.0, st.fg),
            RowKind::Item => (st.font_size, 400.0, st.fg),
        }
    }
}

impl<S: 'static> Node for NavRow<S> {
    fn layout(&mut self, cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let (px, weight) = (style[i].font_size, style[i].font_weight);
        let key = MeasureKey::new(cx, limits, &style[i]);
        let label = self.label;
        let text = self.measured.get_or(key, label, || cx.text.measure_weighted(label, px, weight, limits.max_w));
        let content = Size { w: text.w + 2.0 * (INSET + TEXT_X), h: text.h };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let pill = [r[0] + INSET, r[1] + 2.0, r[2] - INSET, r[3] - 2.0];
        if self.kind == RowKind::Item {
            out.rounded_rect(pill, s.bg, s.radius);
            if self.selected {
                let a = s.border_color;
                out.rounded_rect(pill, Color(a.0, a.1, a.2, SELECTED_TINT_ALPHA), s.radius);
                let (y0, y1) = (pill[1] + 7.0, pill[3] - 7.0);
                out.rounded_rect([pill[0] + 3.0, y0, pill[0] + 6.0, y1], a, 1.5);
            }
        }
        let (px, weight, color) = self.face(s);
        let h = self.measured.size().h.max(px);
        let x = pill[0] + TEXT_X;
        let y = r[1] + ((r[3] - r[1]) - h).max(0.0) / 2.0;
        // A heading sits on the row's lower half: it labels what follows.
        let y = if self.kind == RowKind::Heading { y + 4.0 } else { y };
        out.text_weighted([x, y, pill[2], y + h], self.label, px, weight, color);
    }

    #[cfg(feature = "agent")]
    fn probe(&mut self, cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let text = cx.text.measure_weighted(self.label, style[i].font_size, style[i].font_weight, limits.max_w);
        let content = Size { w: text.w + 2.0 * (INSET + TEXT_X), h: text.h };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }

    lumen_static::leaf_common!(NavRow<S>, 0);

    #[cfg(feature = "agent")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        match self.kind {
            RowKind::Heading => node.set_role(lumen_static::observe::accesskit::Role::Heading),
            RowKind::Item => {
                node.set_role(lumen_static::observe::accesskit::Role::Tab);
                node.add_action(lumen_static::observe::accesskit::Action::Click);
                if self.selected {
                    node.set_selected(true);
                }
            }
        }
        node.set_label(self.label);
    }
}

impl<S: 'static> Handlers<S> for NavRow<S> {
    /// **A heading has no handler**, so a press on one is not handled at
    /// all — `pump` answers `handled == false`, which is what the test for
    /// it asserts.
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        match self.kind {
            RowKind::Heading => None,
            RowKind::Item => Some(self.on_click),
        }
    }
}
