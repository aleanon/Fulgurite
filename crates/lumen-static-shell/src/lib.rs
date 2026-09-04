//! The host: the frame loop, a software renderer, and the surface an
//! agent drives — plan T5.3.
//!
//! This is everything between an application and the operating system
//! *except* the OS binding itself. A winit front end adds one thing:
//! translating window events into [`Host::post`] and blitting
//! [`Canvas::pixels`](software::Canvas). Keeping that seam explicit is what lets the loop be
//! tested headlessly, which is how every other subsystem here got its
//! acceptance test.

use lumen_static::cx::{BlockMetrics, LayoutCx, TextMeasure};
use lumen_static::desc::NodeState;
use lumen_static::display::{Cmd, Color, DisplayList, Renderer};
use lumen_static::event::App;
use lumen_static::style::{Rule, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::retain::{Damage, Incremental, Retention};
use lumen_static::{pass, Constraints, Event, Point};

pub mod software;
#[cfg(feature = "truetype")]
pub mod text;

/// A running application: the tree, its parallel arrays, the one input
/// queue, and the frame's display list.
pub struct Host<A: App, R: Retention = Incremental> {
    pub app: A,
    pub styles: Vec<Style>,
    pub states: Vec<NodeState>,
    pub list: DisplayList,
    pub focus: Option<u32>,
    /// The node that owns the current pointer gesture (M11/T11.2).
    ///
    /// Set when a node consumes a `PointerDown`, cleared on the release.
    /// While it is set, moves go straight to that node and nothing walks
    /// — which is what lets a slider keep tracking after the pointer has
    /// left it without reinstating the per-motion ancestor walk M9
    /// refused.
    pub capture: Option<u32>,
    queue: lumen_static::InputQueue,
    /// Surface size in **physical** pixels — what a window reports.
    size: (u32, u32),
    /// Physical pixels per logical pixel (M10b).
    ///
    /// Everything above the renderer is logical: constraints, `Style`,
    /// `NodeState`, hit testing and every bound an agent reports. Only
    /// the display list is physical, produced by wrapping the painter in
    /// `Scaled`. Without this the framework laid out against *physical*
    /// pixels, so a window rendered half-size on a 2x display and a
    /// 24-logical-pixel accessibility floor was measured against numbers
    /// twice too large.
    scale: f32,
    frame: u64,
    last_layout_ns: u64,
    last_paint_ns: u64,
    /// How much of the previous frame is kept, and what changed
    /// (`docs/plan-architecture-v2.md` M8b). A type parameter rather
    /// than a feature so `Rebuild` stays available in the same binary
    /// as the oracle every incremental gate is compared against.
    retain: R,
    last_damage: Damage,
}

impl<A: App, R: Retention> Host<A, R> {
    pub fn new(app: A, width: u32, height: u32) -> Self {
        let n = <A::Tree as Widget>::NODE_COUNT;
        let styles = pass::default_styles(app.tree());
        Host {
            app,
            styles,
            states: vec![NodeState::default(); n],
            list: DisplayList::new(),
            focus: None,
            capture: None,
            queue: lumen_static::InputQueue::new(),
            size: (width, height),
            scale: 1.0,
            frame: 0,
            last_layout_ns: 0,
            last_paint_ns: 0,
            retain: R::default(),
            last_damage: Damage::All,
        }
    }

    /// Apply a stylesheet. Re-runs the cascade, which is the only pass
    /// that walks the table linearly and the only one that does not run
    /// every frame.
    pub fn set_stylesheet(&mut self, sheet: &[Rule]) {
        // `cascade_deep`, not `cascade`: a rule must be able to reach a
        // repeater's template row or an `Either` branch, which are not
        // in the table (`docs/plan-review-fixes.md` F4.5).
        lumen_static::style::cascade_deep(self.app.tree_mut(), sheet, &mut self.styles);
    }

    /// Re-cascade one subtree, by the `#[widget_id]` on its root.
    ///
    /// The operation a class toggle performs — the tally swaps two
    /// rules when the sign flips — and the reason it is worth scoping:
    /// a full `set_stylesheet` re-resolves every node against every
    /// rule, measured at **36 µs for 901 nodes**, four times what a
    /// whole-tree layout and paint cost there
    /// (`docs/results-cascade-cost.md`).
    ///
    /// `sheet` must differ from the last one cascaded only in rules that
    /// match inside the subtree; debug builds check that against a full
    /// cascade (`style::cascade_subtree`). Returns whether the id was
    /// found — a typo'd id is a silent no-op otherwise, which is the
    /// failure mode this whole crate keeps finding.
    #[must_use = "a false return means the id matched no node and nothing was restyled"]
    pub fn restyle_subtree(&mut self, sheet: &[Rule], wid: &str) -> bool {
        let table = <A::Tree as Widget>::SUB_TABLE;
        let Some(root) = lumen_static::style::index_of_wid(table, wid) else {
            return false;
        };
        lumen_static::style::cascade_subtree(self.app.tree_mut(), sheet, &mut self.styles, root);
        true
    }

    /// A new viewport. Everything is laid out against it, so nothing
    /// retained from the previous size describes the next frame:
    /// retention is reset and the next frame is a full redraw.
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.size == (width, height) {
            return;
        }
        self.size = (width, height);
        self.retain.reset();
        self.last_damage = Damage::All;
    }

    /// Queue one input. OS input, agent-synthesized input and test input
    /// all arrive here — the single queue that makes a session replay.
    pub fn post(&mut self, ev: Event) {
        self.queue.push(ev);
    }

    pub fn frame_number(&self) -> u64 {
        self.frame
    }

    pub fn timings(&self) -> (u64, u64) {
        (self.last_layout_ns, self.last_paint_ns)
    }

    /// Logical viewport: the physical surface divided by the scale.
    fn limits(&self) -> Constraints {
        Constraints {
            min_w: 0.0,
            max_w: self.size.0 as f32 / self.scale,
            min_h: 0.0,
            max_h: self.size.1 as f32 / self.scale,
        }
    }

    /// Physical pixels per logical pixel. 1.0 until a shell says
    /// otherwise.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Tell the host what the display's scale factor is — from
    /// `winit`'s `Window::scale_factor`, and again on
    /// `ScaleFactorChanged`.
    ///
    /// Resets retention: every laid-out position changes, so nothing
    /// kept from the previous frame describes the next one.
    pub fn set_scale(&mut self, scale: f32) {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        if self.scale == scale {
            return;
        }
        self.scale = scale;
        self.retain.reset();
        self.last_damage = Damage::All;
    }

    /// A physical point (what an OS reports) in the logical coordinates
    /// everything above the renderer uses.
    ///
    /// A shell must convert before `post`ing a pointer event, or clicks
    /// land at twice their coordinates on a 2x display.
    pub fn to_logical(&self, x: f32, y: f32) -> (f32, f32) {
        (x / self.scale, y / self.scale)
    }

    /// One frame: drain input, lay out, paint. Returns whether any event
    /// was consumed, which a real shell uses to decide whether to
    /// redraw.
    pub fn pump(&mut self) -> bool {
        self.pump_with(&mut BlockMetrics)
    }

    /// Same, with a caller-supplied text stack.
    pub fn pump_with(&mut self, metrics: &mut dyn TextMeasure) -> bool {
        let mut handled = false;
        while let Some(ev) = self.queue.pop() {
            handled |= pass::dispatch(
                &mut self.app,
                &self.styles,
                &mut self.states,
                &mut self.focus,
                &mut self.capture,
                &ev,
            );
        }

        // Derived presentation is refreshed here, between input and
        // layout: a handler has just changed state, and layout is about
        // to measure what that state renders as. Doing it in the loop
        // rather than at every call site is what makes a binding
        // impossible to forget.
        self.app.update();

        let limits = self.limits();
        let t0 = std::time::Instant::now();
        {
            let mut cx = LayoutCx::new(metrics);
            let tree = self.app.tree_mut();
            tree.layout(&mut cx, limits, &self.styles, &mut self.states, 0);
            tree.place(Point::default(), &self.styles, &mut self.states, 0);
        }
        self.last_layout_ns = t0.elapsed().as_nanos() as u64;

        let t1 = std::time::Instant::now();
        self.list.clear();
        {
            // Logical geometry in, physical commands out (M10b), with
            // retention observing the stream on the way past (T13.8).
            // At a scale of 1.0 `Scaled` is a forwarding wrapper, and
            // under `Rebuild` `Recording` forwards to an empty
            // `observe` the optimizer removes.
            let mut scaled = lumen_static::Scaled::new(&mut self.list, self.scale);
            let mut out = lumen_static::Recording::new(&mut scaled, &mut self.retain);
            self.app.tree().paint(&self.states, 0, &mut out);
        }
        self.last_paint_ns = t1.elapsed().as_nanos() as u64;

        // What this frame changed. Retention has already seen every
        // command; this closes the frame. Free under `Rebuild`.
        self.last_damage = self.retain.finish();

        self.frame += 1;
        handled
    }

    /// What the frame just pumped changed, against the frame before it.
    pub fn damage(&self) -> Damage {
        self.last_damage
    }

    /// What a surface buffer `age` frames old needs redrawn, and bank
    /// the accumulated damage as presented.
    ///
    /// The question a shell presenting into a multi-buffered surface
    /// must ask instead of [`Host::damage`]: `age` comes from the
    /// surface — softbuffer reports it per buffer — 0 meaning its contents are
    /// unspecified. Call it once per presented frame — it counts
    /// presents, and a shell that pumps twice for one frame (a
    /// stylesheet swap between passes) must not call it twice.
    pub fn take_damage_for_age(&mut self, age: u8) -> Damage {
        self.retain.take_damage_for_age(age)
    }

    /// Hand the frame to a renderer. The renderer is chosen by the
    /// caller and never appears in a widget signature.
    ///
    /// The list is already in physical pixels — `pump` painted it
    /// through `Scaled` — so a renderer needs to know nothing about the
    /// scale factor.
    pub fn render(&self, r: &mut dyn Renderer) {
        r.render(&self.list, self.size.0, self.size.1);
    }

    /// Convenience: click at a point, as two ordinary queued events.
    pub fn click(&mut self, x: f32, y: f32) {
        self.post(Event::PointerDown { x, y });
        self.post(Event::PointerUp { x, y });
    }

    /// The node under a point, for tests and tooling.
    pub fn probe(&self, x: f32, y: f32) -> Option<lumen_static::Hit> {
        pass::hit_test(self.app.tree(), &self.states, x, y)
    }
}

/// Background colour a shell clears to before rendering.
pub const CLEAR: Color = Color(255, 255, 255, 255);

/// Everything a command list needs to be drawn, for renderers that want
/// to walk it without matching on `Cmd` themselves.
pub fn for_each_rect(list: &DisplayList, mut f: impl FnMut([f32; 4], Color)) {
    for c in &list.cmds {
        if let Cmd::Rect { rect, color } = c {
            f(*rect, *color);
        }
    }
}
