//! **An application that panics where you ask it to** — M36/T36.0
//! (`docs/plan-panic-recovery.md`).
//!
//! Seven sites, each a place a frame runs application code. Each panic
//! line carries a `// SITE n` marker, and [`site_line`] finds it by
//! scanning this file's own source — so a test compares a report's line
//! with the real one and nobody copies a number. (The first version
//! recorded `line!() + k` constants and four of seven were off by a line
//! or three: hand-counted offsets were exactly the copying this avoids.)

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{NodeDesc, NO_PARENT};
use lumen_static::event::{App, EventCtx, Handlers, NodeSel};
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, JobFn};
use lumen_static::style::{Seed, Style};
use lumen_static::{Constraints, NodeState, Painter};
use lumen_static_widgets::prelude::*;

/// Where each site's panic is raised.
pub const SITES: [(u8, &str); 7] = [
    (1, "a click handler"),
    (2, "an applied message"),
    (3, "update (a binding's source)"),
    (4, "a widget's layout"),
    (5, "a widget's paint"),
    (6, "App::tick"),
    (7, "the framework, called wrongly by a handler"),
];

/// A leaf that panics in `layout` or `paint` when armed — the fixture's
/// own widget, because no shipped widget can be made to.
#[derive(Default)]
pub struct Bomb {
    pub arm: u8,
}

impl Widget for Bomb {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "bomb";
    fn type_defaults() -> Style {
        Style::default()
    }
    const SEEDS: &'static [Seed] = &[Seed::new(
        Self::KIND,
        Self::type_defaults,
        lumen_static::style::Fields::of(&[lumen_static::style::Fields::VISIBLE]),
    )];
}

impl Node for Bomb {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let size = Size { w: 10.0, h: 10.0 };
        let _ = limits;
        if self.arm == 4 {
            panic!("site 4: a widget's layout"); // SITE 4
        }
        lumen_static::ops::record_painted(st, i, limits, size, &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        out.rect(lumen_static::ops::rect_of(&st[i]), st[i].bg);
        if self.arm == 5 {
            let _ = st;
            panic!("site 5: a widget's paint"); // SITE 5
        }
    }
    lumen_static::leaf_common!(Bomb, 0);
}

impl<S> Handlers<S> for Bomb {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
pub struct Ui {
    #[widget_id("title")]
    pub title: Label<String>,
    #[widget_id("go")]
    pub go: Button<Fx>,
    #[widget_id("bomb")]
    pub bomb: Bomb,
}

/// What `update` formats: a value whose `Display` panics when armed.
pub struct Shown(pub u8);

impl core::fmt::Display for Shown {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == 3 {
            panic!("site 3: update, formatting a binding"); // SITE 3
        }
        write!(f, "armed {}", self.0)
    }
}

/// The application. Hand-written, because site 2 needs a real `Msg`.
#[derive(Default)]
pub struct Fx {
    pub ui: Ui,
    /// Which site will panic next, if any.
    pub armed: u8,
    pub outbox: lumen_static::spawn::Outbox,
    pub clicks: u32,
}

fn echo(d: Descriptor, tx: &lumen_static::inbox::Sender<u8>) {
    let _ = tx.send(d.payload as u8);
}

/// A job that panics — trigger 8. M28's spawner catches it as
/// `Failure::Panicked`; the frame guard must not report it (M36/T36.1).
fn bad_job(_: Descriptor, _: &lumen_static::inbox::Sender<u8>) {
    panic!("a job's panic, which the spawner owns");
}

fn on_go(s: &mut Fx, _: &EventCtx) {
    s.clicks += 1;
    if s.armed == 1 {
        panic!("site 1: a click handler"); // SITE 1
    }
    if s.armed == 2 {
        s.outbox.push(Descriptor::new(1, 2));
    }
    if s.armed == 8 {
        s.outbox.push(Descriptor::new(8, 0));
    }
    if s.armed == 7 {
        // An index the framework trusts the caller about: the panic is
        // raised inside `lumen-static`, and the application's line is this.
        let _ = lumen_static::style::subtree_range(<Ui as Widget>::SUB_TABLE, 10_000); // SITE 7
    }
}

impl App for Fx {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = Ui;
    type Msg = u8;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn outbox(&mut self) -> Option<&mut lumen_static::spawn::Outbox> {
        Some(&mut self.outbox)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u8>> {
        Some(if d.kind == 8 { bad_job } else { echo })
    }
    fn apply(this: &mut Self, m: u8) -> bool {
        if m == 2 && this.armed == 2 {
            panic!("site 2: an applied message"); // SITE 2
        }
        true
    }
    fn tick(&mut self, dt_ms: f32) -> bool {
        if dt_ms > 0.0 && self.armed == 6 {
            panic!("site 6: App::tick"); // SITE 6
        }
        false
    }
    fn update(&mut self) -> bool {
        let text = format!("{}", Shown(self.armed));
        let a = lumen_static::bind::set_if_changed(&mut self.ui.title, &text);
        let bomb = if matches!(self.armed, 4 | 5) { self.armed } else { 0 };
        let b = lumen_static::bind::BindValue::bind_value(&mut self.ui.bomb.arm, &bomb);
        a | b
    }
}

/// The application, built.
pub fn build() -> Fx {
    let mut f = Fx::default();
    f.ui.go.label = "go";
    f.ui.go.on_click = on_go;
    f
}

/// The line each site's panic is raised on, in this file — for site 7,
/// the application's calling line, since the panic itself is inside the
/// framework. Found from the `// SITE n` markers in this file's source.
pub fn site_line(site: u8) -> u32 {
    let marker = format!("// SITE {site}");
    include_str!("lib.rs")
        .lines()
        .position(|l| l.trim_end().ends_with(&marker))
        .map_or(0, |i| i as u32 + 1)
}
