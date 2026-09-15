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
use crate::style::{Edges, Length, Palette, Seed, Status, Style};
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
    /// How it looks, as a function of the palette and its pointer state
    /// — M25/T25.5.
    ///
    /// **Code, not data**, and a hot reload treats it as such: `fresh()`
    /// supplies the new build's function and `restore` never touches it,
    /// the same as `on_toggle`. [`default_look`] unless an application
    /// sets one; kind-wide theming is the application's
    /// [`Palette`], not a look per instance.
    pub look: fn(&Palette, Status) -> ToggleLook,
    /// The look as last resolved in `layout`, which `paint` reads.
    /// Private: `layout` overwrites it every frame, so a value set from
    /// outside would last one frame at most.
    resolved: ToggleLook,
}

pub type CheckBox<S> = Toggle<S, CHECKBOX>;
pub type Radio<S> = Toggle<S, RADIO>;
pub type Switch<S> = Toggle<S, SWITCH>;

/// Every colour a toggle paints — M25/T25.5.
///
/// The three that `Style` could not hold on every node, and the one that
/// moves off it: a **mark** colour (the tick, the dot, the knob), which
/// was the hard-coded white `MARK` because a third colour per node was
/// too dear; a **focus** colour, which was the node's `fg`; and a body
/// for each state, which were `background` and `color`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToggleLook {
    /// The body when off.
    pub body: Color,
    /// The body when on.
    pub on_body: Color,
    /// The tick, the dot, the switch's knob.
    pub mark: Color,
    /// The keyboard focus ring.
    pub focus: Color,
}

/// The look a toggle has unless an application gives it one.
///
/// The palette's `control` grey off and `accent` on, with an
/// `on_accent` mark — the colours this widget has always had under the
/// light palette, so an application that sets nothing sees no change.
/// Hovered and pressed darken both bodies by the amounts the cascade's
/// `hover_dim` and `pressed_dim` used to (7% and 18%).
pub fn default_look(p: &Palette, status: Status) -> ToggleLook {
    let k = match status {
        Status::Pressed => 0.18,
        Status::Hovered => 0.07,
        Status::Active | Status::Disabled => 0.0,
    };
    ToggleLook {
        body: crate::ops::dim(p.control, k),
        on_body: crate::ops::dim(p.accent, k),
        mark: p.on_accent,
        focus: p.focus,
    }
}

impl<S, const SHAPE: u8> Default for Toggle<S, SHAPE> {
    fn default() -> Self {
        Toggle {
            on: false,
            on_toggle: crate::ops::ignore_click::<S>,
            look: default_look,
            resolved: default_look(&Palette::LIGHT, Status::Active),
        }
    }
}

impl<S, const SHAPE: u8> Toggle<S, SHAPE> {
    /// The look `layout` last resolved — what `paint` is drawing.
    pub fn resolved_look(&self) -> ToggleLook {
        self.resolved
    }

    /// Replace the look.
    ///
    /// M25/T25.12 adds this to the pilot so `benches --bin appearance` can
    /// install a counting look through the same call on the pilot and on
    /// the cached variant. Here it only assigns.
    pub fn set_look(&mut self, look: fn(&Palette, Status) -> ToggleLook) {
        self.look = look;
    }
}

impl<S: 'static, const SHAPE: u8> Widget for Toggle<S, SHAPE> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    /// Its size comes from its style, never from content.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = match SHAPE {
        RADIO => "radio",
        SWITCH => "switch",
        _ => "checkbox",
    };

    /// Its size, corner and cursor. **Not its colours**, since
    /// M25/T25.5: those are its [`ToggleLook`], resolved from the
    /// palette, and a stylesheet's `background`, `color` or dims no
    /// longer reach it — the sheet lint reports a rule that tries.
    ///
    /// The two defects the colour defaults here once guarded against
    /// stay guarded, by [`default_look`] and [`Palette::control`]: an "on"
    /// switch with a transparent, invisible knob, and an unchecked box in
    /// a grey sixteen levels from a light page.
    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(if SHAPE == SWITCH { TRACK_W } else { BOX }),
            height: Length::Fixed(BOX),
            radius: if SHAPE == RADIO { BOX / 2.0 } else { 5.0 },
            padding: Edges::all(0.0),
            cursor: crate::style::Cursor::Pointer,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults).reading(crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::CURSOR,
            crate::style::Fields::FOCUS_RING,
            crate::style::Fields::VISIBLE,
            crate::style::Fields::RADIUS,
        ]))];
}

/// Logical pixels. 24 is the WCAG 2.5.8 minimum pointer target, and a
/// control that ships below its own accessibility floor is a control
/// whose default is wrong (`scripts/ui_check.py targets`).
const BOX: f32 = 24.0;
const TRACK_W: f32 = 40.0;

impl<S: 'static, const SHAPE: u8> Node for Toggle<S, SHAPE> {
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        // M25/T25.5: the look, against the host's palette and this node's
        // pointer state. Resolved here and not in `paint`, which has no
        // palette — and correct under `Boundaries` only because a hover
        // or press marks the node (T25.0), so this runs when they change.
        self.resolved = (self.look)(cx.palette, Status::of(&st[i]));
        let size = resolve_self(limits, &me, Size { w: BOX, h: BOX });
        record_painted(st, i, limits, size, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let look = self.resolved;
        crate::ops::focus_ring_in(st, i, look.focus, out);
        let s = &st[i];
        let r = rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        // The body is the look's for the state it is in, and the mark is
        // the look's `mark` in both. Nothing draws a mark when off — a
        // checkbox has no tick, a radio no dot — and the switch's knob is
        // drawn in both states, so it takes the same `mark`.
        let (body, mark) = if self.on { (look.on_body, look.mark) } else { (look.body, look.mark) };
        match SHAPE {
            SWITCH => {
                out.rounded_rect(r, body, h / 2.0);
                // The knob travels; its diameter is the track's height
                // less a hairline, so it reads as inside the track.
                //
                // **The knob is always light, and does not invert with
                // the track.** The `(body, mark)` swap above is right
                // for a checkbox and a radio, where the mark exists only
                // when the control is on — so the two colours really do
                // cover four states. A switch is the odd one out: its
                // knob is drawn in *both* states, so inverting put the
                // accent colour on the knob of an **off** switch, which
                // reads as on. Every toolkit draws a light knob on a
                // grey track and a light knob on a coloured one, and
                // that is what this does.
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

    #[cfg(feature = "observe")]
    fn look(&self, out: &mut dyn FnMut(&'static str, Color)) {
        let l = self.resolved;
        out("body", l.body);
        out("on_body", l.on_body);
        out("mark", l.mark);
        out("focus", l.focus);
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
