//! Checkbox, radio and switch — plan M11/T11.1.
//!
//! One type with three presentations, chosen by a **const** parameter
//! rather than a field. That matters here: `kind_name` feeds the
//! cascade and the agent's selectors, and `defaults` is snapshotted once
//! when a stylesheet is set. A shape held in a field would make both
//! state-dependent — the hazard `Option::defaults` was fixed for
//! (`docs/plan-review-fixes.md` F3.2) — so `Toggle<S, CHECKBOX>` and
//! `Toggle<S, SWITCH>` are different types that answer differently at
//! compile time.
//!
//! # `on` is a presentation copy, not the truth
//!
//! A toggle does **not** flip itself. The application owns the boolean
//! and `#[bind]` keeps this copy in step, exactly as a `Label`'s text is
//! a copy of application state
//! (`docs/layout-and-storage-decisions.md` §1). A click runs the app's
//! handler, the handler flips the app's field, and the binding writes
//! the result back here before the frame that shows it.
//!
//! That is why there is no `on_event` impl: a widget that flipped itself
//! would consume the event (C3's contract is that `true` stops the
//! walk), the app's handler would never run, and the two copies would be
//! free to disagree. The one place the widget owning state *is* right is
//! a text field, where the data originates at the widget because the
//! user typed it — which is B1, pointing the other way for the same
//! reason.
//!
//! # Radio
//!
//! A radio's "am I selected" is a comparison against a shared value, so
//! the selection lives in application state and each radio binds to it
//! with a `with =` function. That costs one small function per option
//! and it is the honest cost of not inventing a group container
//! (`docs/plan-widgets.md` T11.1 records the alternative).

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "observe")]
use alloc::vec::Vec;

use crate::desc::{NodeDesc, NodeState, NO_PARENT};
use crate::display::{Color, Painter};
use crate::event::{EventCtx, NodeSel};
use crate::hit::{contains, Hit};
use crate::ops::{record_painted, rect_of, resolve_self};
use crate::style::{Edges, Length, Style};
use crate::widget::{Node, Widget};
use crate::{Constraints, LayoutCx, Point, Size};

/// A square box with a tick.
pub const CHECKBOX: u8 = 0;
/// A circle with a dot — one of a set.
pub const RADIO: u8 = 1;
/// A sliding pill.
pub const SWITCH: u8 = 2;

/// A two-state control. See the module docs for why `SHAPE` is a const
/// and `on` is a copy.
pub struct Toggle<S: 'static, const SHAPE: u8 = CHECKBOX> {
    /// Whether it reads as on. A presentation copy — bind it.
    pub on: bool,
    /// Run on activation: click, Enter or Space. Flip the application's
    /// own field here; the binding writes `on` back.
    pub on_toggle: fn(&mut S, &EventCtx),
}

pub type CheckBox<S> = Toggle<S, CHECKBOX>;
pub type Radio<S> = Toggle<S, RADIO>;
pub type Switch<S> = Toggle<S, SWITCH>;

impl<S, const SHAPE: u8> Default for Toggle<S, SHAPE> {
    fn default() -> Self {
        Toggle { on: false, on_toggle: crate::ops::ignore_click::<S> }
    }
}

impl<S: 'static, const SHAPE: u8> Widget for Toggle<S, SHAPE> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    /// Its size comes from its style, never from content.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
}

/// Logical pixels. 24 is the WCAG 2.5.8 minimum pointer target, and a
/// control that ships below its own accessibility floor is a control
/// whose default is wrong (`scripts/ui_check.py targets`).
const BOX: f32 = 24.0;
const TRACK_W: f32 = 40.0;

impl<S: 'static, const SHAPE: u8> Node for Toggle<S, SHAPE> {
    fn kind_name(&self) -> &'static str {
        match SHAPE {
            RADIO => "radio",
            SWITCH => "switch",
            _ => "checkbox",
        }
    }

    /// A visible control with no stylesheet at all.
    ///
    /// Both colours are opaque deliberately. `Style::background`
    /// defaults to transparent, and `rounded_rect` drops a transparent
    /// command — so the first version of this widget, which inverted
    /// `fg`/`bg` for the on state, drew an "on" switch with **no knob**:
    /// the mark was the transparent default. A control whose default
    /// styling is invisible in one of its two states is not a default.
    fn defaults(&self) -> Style {
        Style {
            width: Length::Fixed(if SHAPE == SWITCH { TRACK_W } else { BOX }),
            height: Length::Fixed(BOX),
            radius: if SHAPE == RADIO { BOX / 2.0 } else { 5.0 },
            padding: Edges::all(0.0),
            // `background` is the resting body, `color` the on body —
            // and each is the other's mark, so two colours cover four
            // states. A stylesheet writing `color: accent, background:
            // white` gets a blue box with a white tick for free.
            background: Color(226, 232, 240, 255),
            color: ON,
            pressed_dim: 0.18,
            hover_dim: 0.07,
            cursor: crate::style::Cursor::Pointer,
            ..Style::default()
        }
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
        let size = resolve_self(limits, &me, Size { w: BOX, h: BOX });
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        // `fg` is the mark, `bg` the resting body. An "on" control fills
        // with `fg` and marks in `bg`, which is the inversion every
        // toolkit uses and which keeps a single pair of colours doing
        // both jobs — so a stylesheet that sets two colours gets four
        // states.
        let (body, mark) = if self.on { (s.fg, s.bg) } else { (s.bg, s.fg) };
        match SHAPE {
            SWITCH => {
                out.rounded_rect(r, body, h / 2.0);
                // The knob travels; its diameter is the track's height
                // less a hairline, so it reads as inside the track.
                let d = (h - 4.0).max(1.0);
                let x = if self.on { r[2] - d - 2.0 } else { r[0] + 2.0 };
                out.rounded_rect([x, r[1] + 2.0, x + d, r[1] + 2.0 + d], mark, d / 2.0);
            }
            RADIO => {
                out.rounded_rect(r, body, w.min(h) / 2.0);
                if self.on {
                    let inset = w.min(h) * 0.3;
                    out.rounded_rect(
                        [r[0] + inset, r[1] + inset, r[2] - inset, r[3] - inset],
                        mark,
                        (w.min(h) - inset * 2.0) / 2.0,
                    );
                }
            }
            _ => {
                out.rounded_rect(r, body, s.radius);
                if self.on {
                    tick(out, r, mark);
                }
            }
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }

    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut crate::observe::accesskit::Node,
        _extra: &mut Vec<(crate::observe::accesskit::NodeId, crate::observe::accesskit::Node)>,
    ) {
        use crate::observe::accesskit::{Role, Toggled};
        node.set_role(match SHAPE {
            RADIO => Role::RadioButton,
            SWITCH => Role::Switch,
            _ => Role::CheckBox,
        });
        // The state a screen reader announces. Without it a checkbox is
        // read as a checkbox and never as a *checked* one.
        node.set_toggled(if self.on { Toggled::True } else { Toggled::False });
        node.add_action(crate::observe::accesskit::Action::Click);
    }
}

/// A checkmark, stepped along its two diagonals.
///
/// The display list has no path command and no rotation — `Cmd` is
/// rects, text, a shadow and clips — so a diagonal is a staircase of
/// small squares. At a 24 px control that reads as a tick; it is a
/// genuine consequence of the vocabulary, and the first version of this
/// function drew the two strokes as one horizontal and one vertical
/// bar, which read as a blob and is why this comment exists.
///
/// Adding a path command for one glyph would be the wrong trade — but
/// it is the same missing vocabulary an icon needs, and
/// `docs/plan-widgets.md` records that as an unscheduled gap rather
/// than pretending rects are enough for everything.
fn tick(out: &mut dyn Painter, r: crate::display::Rect, mark: Color) {
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let t = (w * 0.13).max(1.5);
    // Normalised: down-right to the corner, then up-right.
    let pts = [(0.26, 0.50), (0.44, 0.68), (0.76, 0.32)];
    let seg = |out: &mut dyn Painter, a: (f32, f32), b: (f32, f32), steps: usize| {
        for k in 0..=steps {
            let f = k as f32 / steps as f32;
            let x = r[0] + (a.0 + (b.0 - a.0) * f) * w;
            let y = r[1] + (a.1 + (b.1 - a.1) * f) * h;
            out.rounded_rect([x - t / 2.0, y - t / 2.0, x + t / 2.0, y + t / 2.0], mark, t / 2.0);
        }
    };
    seg(out, pts[0], pts[1], 4);
    seg(out, pts[1], pts[2], 7);
}

impl<S: 'static, const SHAPE: u8> crate::event::Handlers<S> for Toggle<S, SHAPE> {
    fn handler(&self, sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        (sel.i == 0).then_some(self.on_toggle)
    }
}

/// The colour a toggle reads as "on" with no stylesheet.
pub const ON: Color = Color(53, 99, 239, 255);
