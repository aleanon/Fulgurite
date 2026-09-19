//! A slider — plan M11/T11.2.
//!
//! # The widget owns its value
//!
//! Unlike a [`Toggle`](crate::toggle::Toggle), whose `on` is a copy of
//! application state, a slider's value **originates at the widget**: it
//! is produced by a gesture, the way a text field's text is produced by
//! typing. So it is owned here, and the application reads it —
//! `ui.volume.value()` — which is B1 and B8 of
//! `docs/text-and-scrolling-decisions.md` applied to a second widget.
//!
//! The rule is the same in both cases and only *looks* opposite:
//! whoever can invalidate the derived state owns the storage.
//!
//! # Why this needed a routing change
//!
//! A drag has to keep reaching the control it started on after the
//! pointer has left it, and `PointerMove` fires continuously. M9
//! deliberately did not route moves at all rather than walk the ancestor
//! chain on every motion for a handler that is almost never there (C6).
//! **Pointer capture** answers both: the node that consumes a
//! `PointerDown` is addressed directly for every subsequent move, with
//! no walk, and nothing else sees them. `docs/results-capture-cost.md`
//! has the number.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "observe")]
use alloc::vec::Vec;

use crate::desc::{NodeDesc, NodeState, NO_PARENT};
use crate::display::{Color, Painter};
use crate::event::{Event, EventCtx, Key};
use crate::hit::{contains, Hit};
use crate::ops::{record_painted, rect_of, resolve_self};
use crate::style::{Edges, Length, Seed, Style};
use crate::widget::{Node, Widget};
use crate::{Constraints, LayoutCx, Point, Size};

/// A value in `0.0..=1.0`, dragged or stepped.
pub struct Slider {
    value: f32,
    /// How far an arrow key moves it. A slider only draggable by mouse
    /// is not an accessible slider.
    pub step: f32,
    /// Track width from the last layout.
    ///
    /// `on_event` gets the node's `local` point and nothing else — no
    /// style array, no state array — so the width a fraction is computed
    /// against has to be remembered. `TextInput::pad_left` and
    /// `ScrollView::measured_overflow` exist for the same reason.
    track_w: f32,
}

impl Default for Slider {
    fn default() -> Self {
        Slider { value: 0.0, step: 0.05, track_w: 0.0 }
    }
}

impl Slider {
    pub fn new(value: f32) -> Self {
        Slider { value: clamp01(value), ..Slider::default() }
    }

    /// The current value, `0.0..=1.0` (B8: the application reads it).
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Set it, clamped. Returns whether it moved.
    pub fn set_value(&mut self, v: f32) -> bool {
        let v = clamp01(v);
        let moved = v != self.value;
        self.value = v;
        moved
    }

    /// The value a pointer at `local_x` selects.
    fn at(&self, local_x: f32) -> f32 {
        if self.track_w <= 0.0 {
            return self.value;
        }
        clamp01(local_x / self.track_w)
    }
}

/// `0.0..=1.0`, with NaN mapped to 0.0.
///
/// NaN is special-cased because `f32::clamp` *returns* NaN for it, and a
/// non-finite value would reach `NodeState` through the knob's rect —
/// where every `contains` against it is false and the control silently
/// stops being clickable. `±inf` needs no special case: clamping takes
/// them to the ends, which is what a gesture past either end means.
fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        crate::num::clamp(v, 0.0, 1.0)
    }
}

impl Widget for Slider {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = "slider";

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(H),
            radius: H / 2.0,
            padding: Edges::all(0.0),
            background: Color(226, 232, 240, 255),
            color: Color(53, 99, 239, 255),
            pressed_dim: 0.10,
            hover_dim: 0.05,
            cursor: crate::style::Cursor::Pointer,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::CURSOR,
            crate::style::Fields::FOCUS_RING,
            crate::style::Fields::VISIBLE,
            crate::style::Fields::COLOR,
            crate::style::Fields::BACKGROUND,
            crate::style::Fields::RADIUS,
            crate::style::Fields::PRESSED_DIM,
            crate::style::Fields::HOVER_DIM,
        ]))];
}

/// Logical pixels; 24 is the WCAG 2.5.8 pointer-target floor.
const H: f32 = 24.0;

impl Node for Slider {
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
        let size = resolve_self(limits, &me, Size { w: H * 4.0, h: H });
        self.track_w = size.w;
        record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        crate::ops::focus_ring(st, i, out);
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        // A thin track, the filled part in `color`, and a knob big
        // enough to grab. The track is inset vertically so the knob
        // reads as sitting on it.
        let t = (h * 0.25).max(2.0);
        let mid = r[1] + h / 2.0;
        out.rounded_rect([r[0], mid - t / 2.0, r[2], mid + t / 2.0], s.bg, t / 2.0);
        let x = r[0] + w * self.value;
        if x > r[0] {
            out.rounded_rect([r[0], mid - t / 2.0, x, mid + t / 2.0], s.fg, t / 2.0);
        }
        let d = h * 0.75;
        let kx = crate::num::clamp(x - d / 2.0, r[0], r[2] - d);
        out.rounded_rect([kx, mid - d / 2.0, kx + d, mid + d / 2.0], s.fg, d / 2.0);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    /// C3: the slider tracks its own gesture.
    ///
    /// Consuming the `PointerDown` is what makes `dispatch` capture the
    /// pointer, so every later move arrives here directly — including
    /// the ones after the pointer has left the control, which is the
    /// half a plain hit test cannot do.
    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            Event::PointerDown { .. } => {
                self.set_value(self.at(cx.local.x));
                // Consumed even when the value did not move: the press
                // still claims the gesture, and returning false here
                // would hand the drag to an ancestor.
                true
            }
            Event::PointerMove { .. } => self.set_value(self.at(cx.local.x)),
            Event::Key { key: Key::Left, down: true } => {
                let v = self.value - self.step;
                self.set_value(v)
            }
            Event::Key { key: Key::Right, down: true } => {
                let v = self.value + self.step;
                self.set_value(v)
            }
            Event::Key { key: Key::Home, down: true } => self.set_value(0.0),
            Event::Key { key: Key::End, down: true } => self.set_value(1.0),
            _ => false,
        }
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
        use crate::observe::accesskit::Role;
        node.set_role(Role::Slider);
        node.set_numeric_value(self.value as f64);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node.set_numeric_value_step(self.step as f64);
    }
}

impl<S> crate::event::Handlers<S> for Slider {
    /// None: a slider is not activated, it is *dragged*, and everything
    /// it does it does in `on_event`. The application reads `value()`.
    fn handler(&self, _sel: crate::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}
