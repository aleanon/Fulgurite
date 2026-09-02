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
use lumen_static::{pass, Constraints, Event, Point};

pub mod software;

/// A running application: the tree, its parallel arrays, the one input
/// queue, and the frame's display list.
pub struct Host<A: App> {
    pub app: A,
    pub styles: Vec<Style>,
    pub states: Vec<NodeState>,
    pub list: DisplayList,
    pub focus: Option<u32>,
    queue: lumen_static::InputQueue,
    size: (u32, u32),
    frame: u64,
    last_layout_ns: u64,
    last_paint_ns: u64,
}

impl<A: App> Host<A> {
    pub fn new(app: A, width: u32, height: u32) -> Self {
        let n = <A::Tree as Widget>::NODE_COUNT;
        let styles = pass::default_styles(app.tree());
        Host {
            app,
            styles,
            states: vec![NodeState::default(); n],
            list: DisplayList::new(),
            focus: None,
            queue: lumen_static::InputQueue::new(),
            size: (width, height),
            frame: 0,
            last_layout_ns: 0,
            last_paint_ns: 0,
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

    pub fn resize(&mut self, width: u32, height: u32) {
        self.size = (width, height);
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

    fn limits(&self) -> Constraints {
        Constraints {
            min_w: 0.0,
            max_w: self.size.0 as f32,
            min_h: 0.0,
            max_h: self.size.1 as f32,
        }
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
            handled |= pass::dispatch(&mut self.app, &self.styles, &self.states, &mut self.focus, &ev);
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
        self.app.tree().paint(&self.states, 0, &mut self.list);
        self.last_paint_ns = t1.elapsed().as_nanos() as u64;

        self.frame += 1;
        handled
    }

    /// Hand the frame to a renderer. The renderer is chosen by the
    /// caller and never appears in a widget signature.
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
