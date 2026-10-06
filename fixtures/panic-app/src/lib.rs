//! **An application that panics where you ask it to** — M36/T36.0
//! (`docs/plan-panic-recovery.md`).
//!
//! Ten sites, each a place a frame runs application code — the eighth,
//! a focused field's `caret_rect`, is asked after the frame, by the
//! window placing its IME area (M80/T80.5), the ninth,
//! `App::wake_in_ms`, by the window computing its next deadline
//! (M84/T84.3), and the tenth, `App::images`, by the renderer reading the
//! application's store — asked first inside the frame's guarded entry
//! (M84/T84.6). Each panic
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
pub const SITES: [(u8, &str); 10] = [
    (1, "a click handler"),
    (2, "an applied message"),
    (3, "update (a binding's source)"),
    (4, "a widget's layout"),
    (5, "a widget's paint"),
    (6, "App::tick"),
    (7, "the framework, called wrongly by a handler"),
    (8, "a focused field's caret_rect (the IME area)"),
    (9, "App::wake_in_ms (the window's next deadline)"),
    (10, "App::images (the store the renderer reads)"),
];

/// The node a site's session presses: `#fuse` for site 8, whose panic
/// needs the field focused, and `#go` for every other.
pub fn target(site: u8) -> &'static str {
    if site == 8 {
        "fuse"
    } else {
        "go"
    }
}

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

/// A focusable leaf whose `caret_rect` panics once when armed — site 8
/// (M80/T80.5). The window asks the focused node's caret after every
/// frame (`Session::caret_rect`), outside the frame's own guard, so this
/// is the site that proves that call is guarded too.
///
/// The arm is a `Cell` because `caret_rect` takes `&self`: it disarms
/// itself as it fires, so a retry finds the field focused and answering,
/// and an `update` that ran in between cannot disarm it first.
#[derive(Default)]
pub struct Fuse {
    pub arm: core::cell::Cell<bool>,
}

impl Widget for Fuse {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = "fuse";
    fn type_defaults() -> Style {
        Style::default()
    }
    const SEEDS: &'static [Seed] = &[Seed::new(
        Self::KIND,
        Self::type_defaults,
        lumen_static::style::Fields::of(&[lumen_static::style::Fields::VISIBLE]),
    )];
}

impl Node for Fuse {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        lumen_static::ops::record_painted(st, i, limits, Size { w: 60.0, h: 20.0 }, &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        out.rect(lumen_static::ops::rect_of(&st[i]), st[i].bg);
    }
    fn caret_rect(&self, st: &[NodeState], i: usize) -> Option<lumen_static::display::Rect> {
        if self.arm.replace(false) {
            panic!("site 8: a focused field's caret_rect"); // SITE 8
        }
        let s = st.get(i)?;
        Some([s.pos.x + 4.0, s.pos.y + 2.0, s.pos.x + 5.0, s.pos.y + s.size.h - 2.0])
    }
    lumen_static::leaf_common!(Fuse, 0);
}

impl<S> Handlers<S> for Fuse {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

/// One button per site, so an agent can arm a site by clicking.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
pub struct Arms {
    #[widget_id("arm1")]
    pub a1: Button<Fx>,
    #[widget_id("arm2")]
    pub a2: Button<Fx>,
    #[widget_id("arm3")]
    pub a3: Button<Fx>,
    #[widget_id("arm4")]
    pub a4: Button<Fx>,
    #[widget_id("arm5")]
    pub a5: Button<Fx>,
    #[widget_id("arm6")]
    pub a6: Button<Fx>,
    #[widget_id("arm7")]
    pub a7: Button<Fx>,
    #[widget_id("arm8")]
    pub a8: Button<Fx>,
    #[widget_id("arm9")]
    pub a9: Button<Fx>,
    #[widget_id("arm10")]
    pub a10: Button<Fx>,
}

/// A 120 ms timer and its count — M47/T47.1's panic-path step: a retry
/// must not credit the timers with the time the screen was up.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
pub struct Clock {
    #[widget_id("timer")]
    pub toggle: Button<Fx>,
    #[widget_id("ticks")]
    pub ticks: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
pub struct Ui {
    pub arms: Arms,
    #[widget_id("title")]
    pub title: Label<String>,
    #[widget_id("go")]
    pub go: Button<Fx>,
    #[widget_id("bomb")]
    pub bomb: Bomb,
    #[widget_id("fuse")]
    pub fuse: Fuse,
    pub clock: Clock,
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
    /// Each site disarms itself just before it panics — the window mode
    /// (M36/T36.3), so a retry can succeed without the agent, whose input
    /// is refused while the screen is up, having to disarm anything.
    pub one_shot: bool,
    /// The timer behind `#timer`, and how many times it has landed.
    pub timers: lumen_static::spawn::Timers,
    pub ticks: u32,
    /// Site 9's arm (M84/T84.3): a `Cell`, because `wake_in_ms` takes
    /// `&self`. `update` sets it and the wake disarms it as it fires, as
    /// `Fuse` does for site 8.
    pub wake_arm: core::cell::Cell<bool>,
    /// Site 10's arm (M84/T84.6): a `Cell` for the same reason —
    /// `App::images` takes `&self`.
    pub images_arm: core::cell::Cell<bool>,
}

/// The trigger for `bad_job`: a job's panic, which is the spawner's and
/// not a site.
pub const JOB_PANIC: u8 = 11;

/// The timer's descriptor kind and key, and so the message it lands as.
pub const TICK: u8 = 9;

impl Fx {
    fn fire(&mut self) {
        if self.one_shot {
            self.armed = 0;
        }
    }
}

fn echo(d: Descriptor, tx: &lumen_static::inbox::Sender<u8>) {
    let _ = tx.send(d.payload as u8);
}

/// A job that panics — trigger 11 (8 until M80/T80.5 and 10 until
/// M84/T84.6 gave those numbers to sites). M28's spawner catches it as
/// `Failure::Panicked`; the frame guard must not report it (M36/T36.1).
fn bad_job(_: Descriptor, _: &lumen_static::inbox::Sender<u8>) {
    panic!("a job's panic, which the spawner owns");
}

fn on_go(s: &mut Fx, _: &EventCtx) {
    s.clicks += 1;
    if s.armed == 1 {
        s.fire();
        panic!("site 1: a click handler"); // SITE 1
    }
    if s.armed == 2 {
        s.outbox.push(Descriptor::new(1, 2));
    }
    if s.armed == JOB_PANIC {
        s.outbox.push(Descriptor::new(8, 0));
    }
    if s.armed == 7 {
        s.fire();
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
    fn timers(&mut self) -> Option<&mut lumen_static::spawn::Timers> {
        Some(&mut self.timers)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u8>> {
        Some(if d.kind == 8 { bad_job } else { echo })
    }
    fn apply(this: &mut Self, m: u8) -> bool {
        if m == 2 && this.armed == 2 {
            this.fire();
            panic!("site 2: an applied message"); // SITE 2
        }
        if m == TICK {
            this.ticks += 1;
        }
        true
    }
    fn tick(&mut self, dt_ms: f32) -> bool {
        if dt_ms > 0.0 && self.armed == 6 {
            self.fire();
            panic!("site 6: App::tick"); // SITE 6
        }
        false
    }
    /// Asked after every frame, by the window's deadline (M84/T84.3),
    /// through the session's guard.
    fn wake_in_ms(&self) -> Option<f32> {
        if self.wake_arm.replace(false) {
            panic!("site 9: App::wake_in_ms"); // SITE 9
        }
        None
    }
    /// Asked by every present (M84/T84.6) — and first inside the frame's
    /// guarded entry, which is what catches this.
    fn images(&self) -> Option<&dyn lumen_static::ImageSource> {
        if self.images_arm.replace(false) {
            panic!("site 10: App::images"); // SITE 10
        }
        None
    }
    fn update(&mut self) -> bool {
        let shown = Shown(self.armed);
        let bomb = if matches!(self.armed, 4 | 5) { self.armed } else { 0 };
        if matches!(self.armed, 3..=5) {
            self.fire();
        }
        // Site 8: arm the field, which disarms itself when it fires.
        if self.armed == 8 {
            self.ui.fuse.arm.set(true);
            self.fire();
        }
        // Site 9: arm the wake, which disarms itself when it fires.
        if self.armed == 9 {
            self.wake_arm.set(true);
            self.fire();
        }
        // Site 10: arm the store, which disarms itself when it fires.
        if self.armed == 10 {
            self.images_arm.set(true);
            self.fire();
        }
        let text = format!("{shown}");
        let a = lumen_static::bind::set_if_changed(&mut self.ui.title, &text);
        let b = lumen_static::bind::BindValue::bind_value(&mut self.ui.bomb.arm, &bomb);
        let c = lumen_static::bind::set_if_changed(&mut self.ui.clock.ticks, &format!("{}", self.ticks));
        a | b | c
    }
}

/// The application, built.
pub fn build() -> Fx {
    let mut f = Fx::default();
    f.ui.go.label = "go";
    f.ui.go.on_click = on_go;
    type Arm<'a> = (&'a mut Button<Fx>, &'static str, fn(&mut Fx, &EventCtx));
    let arms: [Arm<'_>; 10] = [
        (&mut f.ui.arms.a1, "1", |s, _| s.armed = 1),
        (&mut f.ui.arms.a2, "2", |s, _| s.armed = 2),
        (&mut f.ui.arms.a3, "3", |s, _| s.armed = 3),
        (&mut f.ui.arms.a4, "4", |s, _| s.armed = 4),
        (&mut f.ui.arms.a5, "5", |s, _| s.armed = 5),
        (&mut f.ui.arms.a6, "6", |s, _| s.armed = 6),
        (&mut f.ui.arms.a7, "7", |s, _| s.armed = 7),
        (&mut f.ui.arms.a8, "8", |s, _| s.armed = 8),
        (&mut f.ui.arms.a9, "9", |s, _| s.armed = 9),
        (&mut f.ui.arms.a10, "10", |s, _| s.armed = 10),
    ];
    for (b, label, h) in arms {
        b.label = label;
        b.on_click = h;
    }
    f.ui.clock.toggle.label = "timer";
    f.ui.clock.toggle.on_click = |s, _| {
        let t = TICK as u64;
        if !s.timers.cancel(t, t) {
            s.timers.every(t, t, 120.0);
        }
    };
    f
}

/// The window mode's application: one-shot sites.
pub fn build_window() -> Fx {
    Fx { one_shot: true, ..build() }
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
