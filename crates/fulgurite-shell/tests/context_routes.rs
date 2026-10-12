//! **Every route to a context menu becomes one event** — M90/T90.3
//! (`docs/plan-desktop-integration.md`, "Every route to a context menu
//! becomes one event"; P29, C28).
//!
//! T90.2 made a right press one `Event::ContextRequest`. This is the
//! other three routes, each ending in the same event:
//!
//! - **the keyboard**: Shift+F10 and the Menu key, at the focused node's
//!   bottom-left (the point under it), bubbling from it — and a key nobody
//!   consumed as a request goes on as a key, so `Program::on_key` still
//!   sees it;
//! - **a touch held still** for `LongPress`'s 500 ms, at the touch point,
//!   with the touch then **spent**: its `End` is a `PointerCancel`, so a
//!   control that activates on release does not; a hold that drifts past
//!   the 10 px slop is still a tap. A press-acting control has already
//!   acted at `Begin` (second review P16; not built, recorded);
//! - **an assistive technology's** `Action::ContextMenu`, inside the
//!   target's rect.
//!
//! Built with `Program::host`, driven by `post`, `tick` and `pump`, under
//! `Everything` and `Boundaries<256>`. Each gate asserts its setup landed
//! and prints its quantity beside its bound.

use std::cell::Cell;

use fulgurite::desc::{NodeDesc, NodeState, NO_PARENT};
use fulgurite::display::Painter;
use fulgurite::event::{EventCtx, Handlers, Mods, NodeSel, TouchPhase};
use fulgurite::invalidate::{Boundaries, Everything, Invalidation};
use fulgurite::ops::{record_painted, resolve_self};
use fulgurite::prelude::*;
use fulgurite::retain::Incremental;
use fulgurite::style::{Seed, Style};
use fulgurite::{Event, Key, LayoutCx};
use fulgurite_shell::program::{application, KeyOutcome, Program};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

const W: u32 = 360;
const H: u32 = 420;

/// A focusable leaf that records each `ContextRequest` it is handed, by
/// its point, and consumes it.
#[derive(Default)]
pub struct Probe {
    requests: Vec<(f32, f32)>,
}

impl Widget for Probe {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = "context-probe";
    const SEEDS: &'static [Seed] = &[Seed::unlinted(Self::KIND, Self::type_defaults)];
}

impl Node for Probe {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let size = resolve_self(limits, &style[i], Size { w: 120.0, h: 40.0 });
        record_painted(st, i, limits, size, &style[i])
    }
    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {}
    fn on_event(&mut self, ev: &Event, _cx: &EventCtx) -> bool {
        match *ev {
            Event::ContextRequest { x, y } => {
                self.requests.push((x, y));
                true
            }
            _ => false,
        }
    }
    fulgurite::leaf_common!(Probe, NodeDesc::FOCUSABLE);
}

impl<S> Handlers<S> for Probe {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("button")]
    button: Button<Count>,
    // Below the button, so its rect does not start at the window's
    // origin: a request synthesised at (0, 0) is outside it.
    #[widget_id("probe")]
    probe: Probe,
    #[widget_id("section")]
    section: Collapsible<Label>,
    #[widget_id("tabs")]
    tabs: Tabs,
}

#[derive(Default)]
struct Count {
    clicks: u32,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    #[state]
    count: Count,
}

thread_local! {
    /// What `on_key` saw: how many times, and the last key and mods.
    static ON_KEY_SEEN: Cell<(u32, Option<(Key, Mods)>)> = const { Cell::new((0, None)) };
}

fn on_key(_: &mut Count, key: Key, mods: Mods) -> KeyOutcome {
    ON_KEY_SEEN.with(|k| {
        let (n, _) = k.get();
        k.set((n + 1, Some((key, mods))));
    });
    KeyOutcome::Ignored
}

fn boot() -> App {
    let mut a = App::default();
    a.ui.button.label = "press";
    a.ui.button.on_click = |c: &mut Count, _: &EventCtx| c.clicks += 1;
    let mut body = Label::default();
    body.set("Body");
    a.ui.section = Collapsible::new("Section", body);
    a.ui.tabs = Tabs::new(&["One", "Two", "Three"]);
    a
}

fn program() -> Program<App> {
    application(boot).size(W, H).on_key(on_key)
}

type Hst<I> = Host<App, Incremental, I>;

fn index(wid: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap_or_else(|| panic!("no #{wid}"))
}

fn host<I: Invalidation>() -> Hst<I> {
    ON_KEY_SEEN.with(|k| k.set((0, None)));
    let mut h: Hst<I> = program().host();
    h.pump();
    h.pump();
    h
}

fn at<I: Invalidation>(h: &Hst<I>, wid: &str, fx: f32, fy: f32) -> (f32, f32) {
    let s = &h.states[index(wid)];
    (s.pos.x + s.size.w * fx, s.pos.y + s.size.h * fy)
}

fn rect<I: Invalidation>(h: &Hst<I>, wid: &str) -> [f32; 4] {
    let s = &h.states[index(wid)];
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

fn key<I: Invalidation>(h: &mut Hst<I>, key: Key, mods: Mods) -> bool {
    h.post(Event::Modifiers(mods));
    h.post(Event::Key { key, down: true });
    h.post(Event::Key { key, down: false });
    h.post(Event::Modifiers(Mods::NONE));
    h.pump()
}

fn keyboard<I: Invalidation>(strategy: &str) {
    for (name, k, mods) in [("Shift+F10", Key::F(10), Mods::SHIFT), ("Menu", Key::Menu, Mods::NONE)] {
        // With a consumer focused: one request, at its bottom-left.
        let mut h = host::<I>();
        h.input.focus = Some(index("probe") as u32);
        h.pump();
        let r = rect(&h, "probe");
        let handled = key(&mut h, k, mods);
        let got = h.app.ui.probe.requests.clone();
        let want = (r[0], r[3]);
        let (seen, _) = ON_KEY_SEEN.with(|c| c.get());
        println!(
            "[{strategy}] {name} focused on #probe {r:?}: handled {handled}, requests {got:?} (want one at the \
             bottom-left {want:?}); on_key saw it {seen} time(s) (want 0)"
        );
        let inside = got.iter().all(|&(x, y)| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]);
        assert!(
            handled && got == vec![want],
            "[{strategy}] {name}: want one request at {want:?}, got {got:?} (on #probe's rect {r:?}: {inside})"
        );
        assert_eq!(seen, 0, "[{strategy}] {name}: a request that was consumed still reached on_key");

        // Accept: focus on a control that consumes no request, and the
        // key goes on as a key, to `on_key`, once.
        let mut h = host::<I>();
        h.input.focus = Some(index("button") as u32);
        h.pump();
        let handled = key(&mut h, k, mods);
        let (seen, last) = ON_KEY_SEEN.with(|c| c.get());
        println!(
            "[{strategy}] {name} focused on #button (no consumer): handled {handled}, on_key saw {seen} time(s) \
             (want 1), last {last:?}; clicks {}",
            h.app.count.clicks
        );
        assert!(seen == 1 && last == Some((k, mods)), "[{strategy}] {name}: on_key saw {seen} time(s), last {last:?}");
        assert_eq!(h.app.count.clicks, 0, "[{strategy}] {name} pressed the button");
    }
}

/// **Shift+F10 and the Menu key ask for a context menu at the focused
/// node**, one request at its bottom-left; with no consumer the key goes
/// on to `on_key`, once.
#[test]
fn shift_f10_and_the_menu_key_ask_at_the_focused_node() {
    keyboard::<Everything>("Everything");
    keyboard::<Boundaries<256>>("Boundaries");
}

/// A primary touch at `(x, y)`, held for `ms` by ticks of `steps`, and
/// optionally dragged by `drift` px; answers the host before the lift.
fn hold<I: Invalidation>(h: &mut Hst<I>, x: f32, y: f32, steps: &[f32], drift: f32) {
    h.post(Event::Touch { id: 1, x, y, phase: TouchPhase::Begin });
    h.pump();
    if drift != 0.0 {
        h.post(Event::Touch { id: 1, x: x + drift, y, phase: TouchPhase::Move });
        h.pump();
    }
    for &dt in steps {
        h.tick(dt);
        h.pump();
    }
}

fn lift<I: Invalidation>(h: &mut Hst<I>, x: f32, y: f32) -> bool {
    h.post(Event::Touch { id: 1, x, y, phase: TouchPhase::End });
    h.pump()
}

fn touch<I: Invalidation>(strategy: &str) {
    // The request: nothing at 499 ms, one at 500, at the touch point.
    let mut h = host::<I>();
    let (x, y) = at(&h, "probe", 0.5, 0.5);
    hold(&mut h, x, y, &[499.0], 0.0);
    let early = h.app.ui.probe.requests.len();
    h.tick(1.0);
    h.pump();
    let got = h.app.ui.probe.requests.clone();
    println!("[{strategy}] hold over #probe at ({x}, {y}): after 499 ms {early} request(s) (want 0); after 500 {got:?} (want one at the point)");
    assert_eq!(early, 0, "[{strategy}] the hold fired before 500 ms");
    assert_eq!(got, vec![(x, y)], "[{strategy}] want one request at ({x}, {y})");
    lift(&mut h, x, y);

    // The lift of a spent touch is a cancel: the button does not activate.
    let mut h = host::<I>();
    let (x, y) = at(&h, "button", 0.5, 0.5);
    hold(&mut h, x, y, &[499.0, 1.0], 0.0);
    let held = h.app.count.clicks;
    lift(&mut h, x, y);
    let lifted = h.app.count.clicks;
    println!("[{strategy}] hold over #button 500 ms: clicks {held} (held) -> {lifted} (lifted; want 0, a cancel)");
    assert_eq!((held, lifted), (0, 0), "[{strategy}] the lift of a long hold activated the button");

    // Accept: a hold that drifts 11 px (slop 10) is a drag, not a hold —
    // no request, and over the button its lift is a tap.
    let mut h = host::<I>();
    let (x, y) = at(&h, "probe", 0.5, 0.5);
    hold(&mut h, x, y, &[499.0, 1.0, 100.0], 11.0);
    let drifted = h.app.ui.probe.requests.len();
    lift(&mut h, x + 11.0, y);
    let mut h2 = host::<I>();
    let (bx, by) = at(&h2, "button", 0.3, 0.5);
    hold(&mut h2, bx, by, &[499.0, 1.0, 100.0], 11.0);
    lift(&mut h2, bx + 11.0, by);
    println!(
        "[{strategy}] a hold drifting 11 px (slop 10): {drifted} request(s) over #probe (want 0); over #button the \
         lift is a tap, clicks {} (want 1)",
        h2.app.count.clicks
    );
    assert_eq!(drifted, 0, "[{strategy}] a drifted hold still asked for a menu");
    assert_eq!(h2.app.count.clicks, 1, "[{strategy}] setup: the drifted touch did not tap the button");
}

/// **A touch held still for 500 ms asks for a context menu at its point,
/// and its lift is a cancel**; a hold that drifts past the slop is a tap.
// ANCHOR: held_touch
#[test]
fn a_touch_held_still_asks_and_its_lift_is_a_cancel() {
    touch::<Everything>("Everything");
    touch::<Boundaries<256>>("Boundaries");
}
// ANCHOR_END: held_touch

fn deadline<I: Invalidation>(strategy: &str) {
    let mut quiet = host::<I>();
    let none = quiet.next_deadline_ms();
    let mut h = host::<I>();
    let (x, y) = at(&h, "probe", 0.5, 0.5);
    hold(&mut h, x, y, &[], 0.0);
    let pending = h.next_deadline_ms();
    h.tick(200.0);
    let later = h.next_deadline_ms();
    lift(&mut h, x, y);
    let after = h.next_deadline_ms();
    println!(
        "[{strategy}] next_deadline_ms: no touch {none:?}; a hold pending {pending:?} (bound <= 500), 200 ms in \
         {later:?}; lifted {after:?} (want {none:?})"
    );
    assert!(pending.is_some_and(|d| d <= 500.0), "[{strategy}] a pending hold set no deadline: {pending:?}");
    assert!(later.is_some_and(|d| d <= 300.0), "[{strategy}] the deadline did not count down: {later:?}");
    assert_eq!(after, none, "[{strategy}] a lifted touch left a deadline");
}

/// **A pending hold is a deadline**, or an idle window would never fire
/// it (the M28/T28.12 lesson); with none, the deadline is unchanged.
#[test]
fn a_pending_hold_is_a_deadline() {
    deadline::<Everything>("Everything");
    deadline::<Boundaries<256>>("Boundaries");
}

fn press_acting<I: Invalidation>(strategy: &str) {
    for (wid, name) in [("section", "Collapsible"), ("tabs", "Tabs")] {
        let mut h = host::<I>();
        let (fx, fy) = if wid == "section" { (0.3, 0.1) } else { (0.5, 0.5) };
        let (x, y) = at(&h, wid, fx, fy);
        let read = |h: &Hst<I>| if wid == "section" { h.app.ui.section.is_open() as u32 } else { h.app.ui.tabs.selected() as u32 };
        let before = read(&h);
        hold(&mut h, x, y, &[499.0, 1.0], 0.0);
        let held = read(&h);
        let capture = h.input.capture.is_some();
        lift(&mut h, x, y);
        let lifted = read(&h);
        println!(
            "[{strategy}] {name} held 500 ms: {before} -> {held} (held) -> {lifted} (lifted); captured while held \
             {capture}, after the lift {} (P16: it acted at Begin, kept)",
            h.input.capture.is_some()
        );
        assert!(before == 0 && held == 1 && lifted == 1, "[{strategy}] {name}: {before} -> {held} -> {lifted}");
        assert!(h.input.capture.is_none() && h.input.press.is_none(), "[{strategy}] {name}: the cancel left a capture or a press");
    }
}

/// **A press-acting control has acted at `Begin`** (second review P16,
/// T90.0's pin kept): the hold cannot undo it, and the lift is a cancel.
#[test]
fn a_press_acting_control_has_acted_before_the_hold() {
    press_acting::<Everything>("Everything");
    press_acting::<Boundaries<256>>("Boundaries");
}

fn assistive<I: Invalidation>(strategy: &str) {
    let mut h = host::<I>();
    let n = index("probe") as u32;
    let r = rect(&h, "probe");
    h.action(n, fulgurite::Action::ContextMenu);
    let handled = h.pump();
    let got = h.app.ui.probe.requests.clone();
    let inside = got.iter().all(|&(x, y)| x >= r[0] && x < r[2] && y >= r[1] && y < r[3]);
    println!("[{strategy}] Action::ContextMenu on #probe {r:?}: handled {handled}, requests {got:?} (want one inside)");
    assert!(handled && got.len() == 1 && inside, "[{strategy}] want one request inside {r:?}, got {got:?}");

    // Accept: on a node that consumes none, nothing is handled and nothing
    // is pressed.
    let mut h = host::<I>();
    h.action(index("button") as u32, fulgurite::Action::ContextMenu);
    let handled = h.pump();
    println!("[{strategy}] Action::ContextMenu on #button: handled {handled} (want false), clicks {}", h.app.count.clicks);
    assert!(!handled && h.app.count.clicks == 0, "[{strategy}] a context-menu action on a button acted");
}

/// **An assistive technology's "show context menu" is the same event**,
/// inside the target's rect.
#[test]
fn an_assistive_technology_asks_inside_the_target() {
    assistive::<Everything>("Everything");
    assistive::<Boundaries<256>>("Boundaries");
}
