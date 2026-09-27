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
#[cfg(feature = "a11y")]
use alloc::vec::Vec;

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::hit::{contains, Hit};
use lumen_static::ops::{record_painted, rect_of, resolve_self};
use lumen_static::style::{Edges, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, Point, Size};

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
    /// What a screen reader announces this control as — M54/T54.3
    /// (R15-F3). Empty by default, and an empty one is not published: a
    /// control with no visible text of its own needs its application to
    /// say what it is for. A field rather than a relation to a sibling
    /// `Label`, so it works inside a `Repeat` row.
    ///
    /// **Only with `a11y`**: a build nobody can read aloud — the firmware
    /// — does not carry it (its `Ui` is a const-asserted size). Write it
    /// through `set_label`, which every build has.
    #[cfg(feature = "a11y")]
    pub label: &'static str,
}

impl Default for Slider {
    fn default() -> Self {
        Slider { value: 0.0, step: 0.05, track_w: 0.0,
            #[cfg(feature = "a11y")]
            label: "",
        }
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
///
/// A *stylesheet* length that is non-finite never gets this far: the
/// cascade repairs it where it enters and the agent reports it as
/// `W1012` (`Patch::needs_repair`, M55/T55.5). This guards the one value no
/// stylesheet carries — a gesture's fraction of the track.
fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        lumen_static::num::clamp(v, 0.0, 1.0)
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
            cursor: lumen_static::style::Cursor::Pointer,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::FOCUS_RING,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::PRESSED_DIM,
            lumen_static::style::Fields::HOVER_DIM,
        ]))];
}

/// Logical pixels; 24 is the WCAG 2.5.8 pointer-target floor.
const H: f32 = 24.0;

impl Node for Slider {
    lumen_static::node_count!();
    /// Arrows move a value, so they never leave this control
    /// (M44/T44.1; [`Node::owns_arrows`]).
    fn owns_arrows(&self) -> bool {
        true
    }

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
        lumen_static::ops::focus_ring(st, i, out);
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
        let kx = lumen_static::num::clamp(x - d / 2.0, r[0], r[2] - d);
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
            // What a screen reader asks (M54/T54.4), one step as an arrow
            // key takes, and published in `observe` — each one here.
            Event::Action { action: lumen_static::Action::Increment, .. } => {
                let v = self.value + self.step;
                self.set_value(v)
            }
            Event::Action { action: lumen_static::Action::Decrement, .. } => {
                let v = self.value - self.step;
                self.set_value(v)
            }
            Event::Action { action: lumen_static::Action::SetValue(v), .. } => self.set_value(*v),
            _ => false,
        }
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }

    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit::Role;
        node.set_role(Role::Slider);
        // The actions `on_event` handles, and no others (M54/T54.4).
        use lumen_static::observe::accesskit::Action as Ak;
        node.add_action(Ak::Increment);
        node.add_action(Ak::Decrement);
        node.add_action(Ak::SetValue);
        if !self.label.is_empty() {
            node.set_label(self.label);
        }
        node.set_numeric_value(self.value as f64);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node.set_numeric_value_step(self.step as f64);
    }
}

impl<S> lumen_static::event::Handlers<S> for Slider {
    /// None: a slider is not activated, it is *dragged*, and everything
    /// it does it does in `on_event`. The application reads `value()`.
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

impl Slider {
    /// Name this control for a screen reader (M54/T54.3). A no-op in a build without `a11y`, which
    /// carries no name.
    #[allow(unused_variables)]
    pub fn set_label(&mut self, label: &'static str) {
        #[cfg(feature = "a11y")]
        {
            self.label = label;
        }
    }
}
