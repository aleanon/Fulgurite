//! M27/T27.7 — the overlay group: `Modal`, `Sheet`, `Drawer`.
//!
//! One type with three presentations, the way `Strip` is — they differ
//! only in where the panel sits and how big it gets:
//!
//! | | panel |
//! |---|---|
//! | `Modal` | centred, sized to its content |
//! | `Sheet` | across the bottom, full width |
//! | `Drawer` | down one side, full height |
//!
//! # The scrim, and why the whole thing defers
//!
//! A modal is two pieces of paint that must land **together and above
//! everything**: a dimming scrim over the window, then the panel on it.
//! `Popup` defers its content and that is enough for a dropdown, which
//! has no scrim — but a wrapper that drew the scrim inline and deferred
//! only the panel would put the dim *under* the content it is meant to
//! dim.
//!
//! So this node defers **itself**: `Painter::defer` records it in phase
//! one and answers `false` in phase two, where scrim and panel are
//! painted in order. A painter with no overlay sink answers `false`
//! immediately and both are painted inline — which is the documented
//! degraded behaviour, not a broken one.
//!
//! # Dismissal is this widget's, not the pass's
//!
//! M15 made dismissal the pass's job for a `Popup` registered in the
//! overlay sink. A veil is modal: a press outside the *panel* is a press
//! on the scrim, which this node owns and must consume — letting it
//! through is precisely the bug a modal exists to prevent.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::LayoutCx;

/// Centred, sized to its content — `Modal`.
pub const MODAL: u8 = 0;
/// Across the bottom, full width — `Sheet`.
pub const SHEET: u8 = 1;
/// Down one side, full height — `Drawer`.
pub const DRAWER: u8 = 2;

/// Which side a drawer comes from — M57/B43. It was always the left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Edge {
    #[default]
    Left,
    Right,
}

/// Upper bound on `1 + W::NODE_COUNT` for one veil.
pub const MAX_VEIL_NODES: usize = 256;

/// A panel over a dimmed window.
pub struct Veil<W: Widget, const KIND: u8 = MODAL> {
    pub panel: W,
    /// Whether it is showing. A closed veil costs a `bool` test, like
    /// `Popup::open`.
    pub open: bool,
    /// Set when a press on the scrim or an Escape asked to close it.
    ///
    /// **A request, not an action.** The widget does not close itself:
    /// an application that must confirm before discarding cannot undo a
    /// close that already happened. `take_dismissed` hands the request
    /// over once — the same take-don't-read shape as
    /// `PullToRefresh::take_triggered`, and for the same reason.
    dismissed: bool,
    /// The panel's measured size, kept from `layout` so `place` can
    /// position it. `Popup::measured` exists for the same reason.
    measured: Size,
    /// A drawer's side (M57/B43). Ignored by the other kinds.
    pub edge: Edge,
    /// The panel's corner radius, from layout: how far a sheet or a
    /// drawer runs past the window so only its inner corners are round.
    bleed: f32,
}

/// A centred panel over a dimmed window.
pub type Modal<W> = Veil<W, MODAL>;
/// A panel across the bottom.
pub type Sheet<W> = Veil<W, SHEET>;
/// A panel down one side.
pub type Drawer<W> = Veil<W, DRAWER>;

impl<W: Widget + Default, const KIND: u8> Default for Veil<W, KIND> {
    fn default() -> Self {
        Veil {
            panel: W::default(),
            open: false,
            dismissed: false,
            measured: Size::default(),
            edge: Edge::Left,
            bleed: 0.0,
        }
    }
}

impl<W: Widget, const KIND: u8> Veil<W, KIND> {
    const AT: usize = 1;
    /// How much of the window a sheet or a drawer takes across its short
    /// axis.
    const SPAN: f32 = 0.42;
    /// The most of the window a **modal's** panel may take.
    ///
    /// **A cap, not a suggestion.** A `Modal<W>` whose `W` is `FILL` —
    /// which most containers are — measures to the whole window, and a
    /// modal with no margin has no scrim: there is nowhere to press to
    /// dismiss it and nothing to show it is above the page. Found by a
    /// test whose panel was a plain `Button`, where a press meant for
    /// the scrim landed on the panel because the panel *was* the window.
    const MODAL_SPAN: f32 = 0.8;
    /// The scrim's alpha. Dark enough to separate, light enough to see
    /// what is behind — every platform lands near 40%.
    const SCRIM: u8 = 0x66;

    /// A sheet's visible height: its content's, within 20–42% of the
    /// window.
    fn sheet_h(&self, h: f32) -> f32 {
        (h * Self::SPAN).min(self.measured.h.max(1.0)).max(h * 0.2)
    }

    pub fn new(panel: W) -> Self {
        Veil { panel, open: false, dismissed: false, measured: Size::default(), edge: Edge::Left, bleed: 0.0 }
    }

    pub fn open(&mut self) -> bool {
        let was = self.open;
        self.open = true;
        !was
    }

    /// Close it. The application calls this; the widget never does.
    pub fn close(&mut self) -> bool {
        let was = self.open;
        self.open = false;
        self.dismissed = false;
        was
    }

    /// Take the pending dismissal request, if there is one.
    pub fn take_dismissed(&mut self) -> bool {
        core::mem::take(&mut self.dismissed)
    }

    /// The panel's rect within a veil laid out at `outer`.
    ///
    /// Public because a test and a drive both need to say "inside the
    /// panel" and "on the scrim" without repeating this arithmetic —
    /// and an assertion that recomputed it could agree with a wrong
    /// implementation.
    ///
    /// **A sheet and a drawer run past the window by their radius** —
    /// M57/B42, B43. Only the corners facing the page should be round;
    /// the panel is laid out that much longer, off the window's edge, and
    /// painted clipped to the veil, so its outer corners are cut off by
    /// the window rather than drawn round. The part of this rect inside
    /// `outer` is what shows.
    pub fn panel_rect(&self, outer: lumen_static::display::Rect) -> lumen_static::display::Rect {
        let (w, h) = (outer[2] - outer[0], outer[3] - outer[1]);
        let b = self.bleed;
        match KIND {
            SHEET => {
                let ph = self.sheet_h(h);
                [outer[0], outer[3] - ph, outer[2], outer[3] + b]
            }
            DRAWER => {
                let pw = w * Self::SPAN;
                match self.edge {
                    Edge::Left => [outer[0] - b, outer[1], outer[0] + pw, outer[3]],
                    Edge::Right => [outer[2] - pw, outer[1], outer[2] + b, outer[3]],
                }
            }
            _ => {
                let (pw, ph) = (self.measured.w.min(w), self.measured.h.min(h));
                let (x, y) = (outer[0] + (w - pw) / 2.0, outer[1] + (h - ph) / 2.0);
                [x, y, x + pw, y + ph]
            }
        }
    }
}

pub struct VeilTable<W, const KIND: u8>(core::marker::PhantomData<W>);

impl<W: Widget, const KIND: u8> VeilTable<W, KIND> {
    const ARRAY: [NodeDesc; MAX_VEIL_NODES] = {
        assert!(W::NODE_COUNT < MAX_VEIL_NODES, "Veil panel exceeds MAX_VEIL_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_VEIL_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_VEIL_NODES] = {
        let mut s = [Seed::WIDGET; MAX_VEIL_NODES];
        s[0] = Seed::new(
            <Veil<W, KIND> as Widget>::KIND,
            <Veil<W, KIND> as Widget>::type_defaults, Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::BACKGROUND,
            Fields::VISIBLE,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.background(lumen_static::style::Role::Scrim));
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

impl<W: Widget, const KIND: u8> Widget for Veil<W, KIND> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// **Rigid**: a veil is the window, whatever is on it. Its panel
    /// growing does not change its own size, so a content change inside
    /// one never relays out what is behind it.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = VeilTable::<W, KIND>::SLICE;
    const KIND: &'static str = match KIND {
        MODAL => "modal",
        SHEET => "sheet",
        _ => "drawer",
    };
    const SEEDS: &'static [Seed] = VeilTable::<W, KIND>::SEED_SLICE;

    /// The whole window, and a dark scrim. A veil with a transparent
    /// background is a panel with nothing separating it from what it
    /// covers — §3b's ghost button, one level up.
    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::FILL,
            background: Color(0x10, 0x14, 0x1c, Veil::<W, KIND>::SCRIM),
            ..Style::default()
        }
    }
}

impl<W: Widget, const KIND: u8> Node for Veil<W, KIND> {
    lumen_static::node_count!();
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
        if !me.visible || !self.open {
            // **Closed is absent, not merely unpainted.** `Option`'s
            // hide, which is what stops a shut modal swallowing every
            // click in the window — a panel that only skipped `paint`
            // would still hit-test at full size.
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        let outer = Size { w: limits.max_w, h: limits.max_h };
        // The panel is measured against what its kind will give it, not
        // against the window: a drawer that measured full-width would
        // wrap its text to a width it never gets.
        let panel_limits = match KIND {
            SHEET => Constraints {
                min_w: outer.w,
                max_w: outer.w,
                min_h: 0.0,
                max_h: outer.h * Self::SPAN,
            },
            DRAWER => Constraints {
                min_w: 0.0,
                max_w: outer.w * Self::SPAN,
                min_h: outer.h,
                max_h: outer.h,
            },
            _ => Constraints {
                min_w: 0.0,
                max_w: outer.w * Self::MODAL_SPAN,
                min_h: 0.0,
                max_h: outer.h * Self::MODAL_SPAN,
            },
        };
        self.measured = self.panel.layout(cx, panel_limits, style, st, i + Self::AT);
        // A sheet or a drawer is laid out again at the size it shows plus
        // its radius — the bleed past the window's edge (see
        // `panel_rect`). Only these two kinds, and only when rounded.
        self.bleed = if KIND == MODAL { 0.0 } else { style[i + Self::AT].radius.max(0.0) };
        if KIND != MODAL {
            let r = self.panel_rect([0.0, 0.0, outer.w, outer.h]);
            let (pw, ph) = (r[2] - r[0], r[3] - r[1]);
            let exact = Constraints { min_w: pw, max_w: pw, min_h: ph, max_h: ph };
            self.panel.layout(cx, exact, style, st, i + Self::AT);
        }
        lumen_static::ops::record_painted(st, i, limits, outer, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        if !self.open {
            return;
        }
        let r = self.panel_rect(lumen_static::ops::rect_of(&st[i]));
        self.panel.place(Point { x: r[0], y: r[1] }, style, st, i + Self::AT);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if !self.open {
            return;
        }
        // Phase 1 records this node and returns; phase 2 — or a painter
        // with no overlay sink — is told `false` and paints here. Both
        // pieces go together, which is the whole reason this defers
        // *itself* rather than letting the panel defer.
        if out.defer(i as u32) {
            return;
        }
        let r = lumen_static::ops::rect_of(&st[i]);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        out.rect(r, st[i].bg);
        // Clipped to the window: a sheet's or a drawer's outer corners
        // and its shadow on that side are cut, not drawn (B42, B43).
        out.push_clip(r);
        self.panel.paint(st, i + Self::AT, out);
        out.pop_clip();
    }

    /// A veil is modal by definition: Tab stays on it (M44/T44.9).
    fn is_modal(&self) -> bool {
        true
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !self.open || !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // **The veil claims everything it covers.** That is what modal
        // means: a press on the scrim must not reach the content behind
        // it. The panel gets first refusal, so what is *on* the veil
        // still works.
        self.panel.hit(st, i + Self::AT, x, y).or(Some(lumen_static::Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        }))
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        if !self.open {
            return false;
        }
        let own = cx.target == cx.node;
        match ev {
            // **A press aimed at this node is a press on the scrim.** One
            // that *bubbled* here from the panel — a label, the panel's
            // own background — is not (M57/A4): this said "anything
            // landing here missed it", and a press on a modal's own title
            // asked to close it (`docs/widget-audit.md` X2).
            Event::PointerDown { .. } if own => {
                self.dismissed = true;
                true
            }
            Event::Key { key: Key::Escape, down: true } => {
                self.dismissed = true;
                true
            }
            // A wheel is swallowed wherever it came from: a wheel over the
            // dialog must not scroll the page behind it.
            Event::Scroll { .. } => true,
            // Anything else aimed at the scrim is swallowed too.
            _ if own => true,
            // **But not what bubbled up from the panel** — M57/C6. Taking
            // it here ended pass 1 before activation ran, so the panel's
            // own Confirm button never activated, by pointer or by Enter.
            // The drive said it did: its counter moved because the press
            // also asked to dismiss, and both bumped one number.
            _ => false,
        }
    }

    /// **Escape, and a press outside, ask to dismiss** — M57/A13
    /// (`docs/plan-widget-fixes.md`).
    ///
    /// The pass's overlay path offers Escape to the topmost overlay through
    /// this, before any widget sees the key. Without an override the veil
    /// answered `false`, so on a freshly opened modal — nothing inside it
    /// focused — Escape reached nothing at all, and worked only once a
    /// control inside had focus and the key bubbled to `on_event`
    /// (`docs/widget-audit.md`, Modal). A veil's dismissal is a *request*,
    /// as its scrim press is: the application answers it through
    /// [`Veil::take_dismissed`], so this raises the flag rather than
    /// closing.
    fn dismiss(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.dismissed = true;
        true
    }

    /// **A press outside the veil is not the veil's to take.** The veil
    /// already claims everything *inside* its box through `hit` — that is
    /// the scrim, and what makes it modal. A press that misses the box
    /// entirely landed on something the veil does not cover, and must
    /// reach it. Found the moment `dismiss` above began answering `true`:
    /// the showcase's modal covers its stage card, not the window, and a
    /// sidebar click beside it was eaten (`every_sidebar_row_picks_its_own_entry`).
    fn dismissal_consumes_press(&self) -> bool {
        false
    }

    /// **And it does not ask to close for it either.** Escape is the only
    /// dismissal the pass asks of a veil; a press on its scrim is the
    /// veil's own, through `on_event`.
    fn dismissed_by_outside_press(&self) -> bool {
        false
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == 0 {
            self
        } else {
            self.panel.node(k - Self::AT)
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == 0 {
            self
        } else {
            self.panel.node_mut(k - Self::AT)
        }
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Dialog);
        node.set_value(if self.open { "open" } else { "closed" });
        if self.open {
            node.set_modal();
        }
    }
}

/// Handlers pass through to the panel; the scrim carries none.
impl<W, S, const KIND: u8> lumen_static::event::Handlers<S> for Veil<W, KIND>
where
    W: Widget + lumen_static::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.panel
            .handler(lumen_static::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}
