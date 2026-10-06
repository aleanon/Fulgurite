//! **The visibility pass** — M85/T85.0's setup half, and T85.3's pass
//! (`docs/plan-layout-input.md`).
//!
//! T85.3 decides a node is visible against the window **and** against the
//! rect of every clipping ancestor. A gate that only ever saw a node enter
//! the window and the viewport on the same frame could not tell those
//! apart: a window-only intersection would pass it (the plan's G18/G20).
//! So the setup is asserted before the pass exists — a `ScrollView` whose
//! viewport is strictly shorter than the window, a target below its fold,
//! wheeled 20 px at a time, and **some step at which the target is inside
//! the window and outside the viewport** — under `Everything` and the
//! default `Boundaries<256>`.
//!
//! The step numbers are recorded once, in `docs/results-layout-input.md`
//! (T85.0 and T85.3).
//!
//! **T85.3** puts a test-only flagged leaf, `Watched`, where the target
//! was, so the mechanism is gated before the widget (T85.4's Pop) exists:
//! a `NodeDesc::WATCHES_VIEW` entry, `Event::Visibility` recorded in
//! `on_event`, and a handler answered only for `NodeSel::visibility`.

use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::retain::Incremental;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Spacer;
use lumen_static_widgets::ScrollView;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Content {
    #[widget_id("filler")]
    filler: Spacer,
    #[widget_id("target")]
    target: Spacer,
    #[widget_id("tail")]
    tail: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("scroll")]
    scroll: ScrollView<Content>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const WINDOW: (u32, u32) = (300, 300);
/// The viewport's height. **The injection** is this at the window's
/// height, which leaves no step with the target in the window and out of
/// the viewport.
const VIEWPORT_H: f32 = 100.0;
/// How far below the viewport's top the target starts: past the window's
/// bottom, so it enters the window before it enters the viewport.
const FILLER_H: f32 = 340.0;
const WHEEL: f32 = 20.0;

fn sheet() -> Vec<Rule> {
    let fixed = |id: &'static str, w: f32, h: f32| {
        Rule::new(Selector::Id(id), Patch::new().width(Length::Fixed(w)).height(Length::Fixed(h)))
    };
    vec![
        fixed("scroll", 200.0, VIEWPORT_H),
        fixed("filler", 200.0, FILLER_H),
        fixed("target", 20.0, 20.0),
        fixed("tail", 20.0, 40.0),
    ]
}

/// `[x0, y0, x1, y1]`.
type Rect = [f32; 4];

fn meets(a: Rect, b: Rect) -> bool {
    a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3])
}

fn rect_of<I: Invalidation>(h: &Host<App, Incremental, I>, wid: &str) -> Rect {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

/// One wheel step's outcome.
#[derive(Debug, PartialEq)]
struct Step {
    handled: bool,
    offset: f32,
    target: Rect,
    in_window: bool,
    in_viewport: bool,
}

/// Wheel the scroller down `WHEEL` px at a time until it stops moving, and
/// answer the viewport, the window and every step.
fn walk<I: Invalidation>() -> (Rect, Rect, Vec<Step>) {
    let mut h: Host<App, Incremental, I> = Host::new(App::default(), WINDOW.0, WINDOW.1);
    h.set_stylesheet(&sheet());
    h.pump();
    let viewport = rect_of(&h, "scroll");
    let window = [0.0, 0.0, WINDOW.0 as f32, WINDOW.1 as f32];
    let (x, y) = ((viewport[0] + viewport[2]) / 2.0, (viewport[1] + viewport[3]) / 2.0);
    let mut steps = Vec::new();
    loop {
        let before = h.app.ui.scroll.scroll;
        h.post(Event::Scroll { x, y, dy: -WHEEL });
        let handled = h.pump();
        let offset = h.app.ui.scroll.scroll;
        if !handled && offset == before {
            break;
        }
        let target = rect_of(&h, "target");
        steps.push(Step {
            handled,
            offset,
            target,
            in_window: meets(target, window),
            in_viewport: meets(target, viewport),
        });
        assert!(steps.len() < 100, "the scroller never stopped");
    }
    (viewport, window, steps)
}

fn check<I: Invalidation>(name: &str) {
    let (viewport, window, steps) = walk::<I>();
    println!("{name}: viewport {viewport:?} (height {}), window {window:?} (height {})", viewport[3] - viewport[1], window[3]);
    for (k, s) in steps.iter().enumerate() {
        println!(
            "  step {:>2}: handled {} offset {:>5} target y {:>5}..{:<5} in window {} in viewport {}",
            k + 1,
            s.handled,
            s.offset,
            s.target[1],
            s.target[3],
            s.in_window,
            s.in_viewport
        );
    }
    assert!(!steps.is_empty(), "setup: no wheel step moved the scroller");
    let mut last = 0.0;
    for (k, s) in steps.iter().enumerate() {
        assert!(s.handled, "step {}: the wheel was not handled", k + 1);
        assert!(s.offset > last, "step {}: the offset did not move ({} after {last})", k + 1, s.offset);
        last = s.offset;
    }
    let window_first = steps.iter().position(|s| s.in_window).map(|k| k + 1);
    let viewport_first = steps.iter().position(|s| s.in_viewport).map(|k| k + 1);
    let gap = steps.iter().position(|s| s.in_window && !s.in_viewport).map(|k| k + 1);
    println!(
        "  first step in the window {window_first:?}, first in the viewport {viewport_first:?}, first inside the window and outside the viewport {gap:?}"
    );
    assert!(
        gap.is_some(),
        "no wheel step left the target inside the window and outside the viewport -- a window-only intersection could not be told from the clip"
    );
    assert!(viewport_first.is_some(), "setup: the target never reached the viewport");
    // Implied by the step above, and asserted for its own sake: T85.3's
    // demo must keep its viewport shorter than its window.
    assert!(
        viewport[3] - viewport[1] < window[3] - window[1],
        "setup: the viewport ({} px) is not shorter than the window ({} px)",
        viewport[3] - viewport[1],
        window[3] - window[1]
    );
}

#[test]
fn a_target_below_the_fold_is_in_the_window_before_it_is_in_the_viewport() {
    check::<Everything>("Everything");
    check::<Boundaries<256>>("Boundaries<256>");
}

/// T85.3's accept direction: a tree with no flagged node lists nothing to
/// watch, so the pass is one empty-slice check per frame.
#[test]
fn a_tree_with_no_flagged_node_watches_nothing() {
    let h: Host<App, Incremental, Boundaries<256>> = Host::new(App::default(), WINDOW.0, WINDOW.1);
    println!("watched views in a tree with no flagged node: {} (bound 0)", h.watched_views());
    assert_eq!(h.watched_views(), 0, "a tree with no WATCHES_VIEW node is watched");
}

// ---- T85.3: the pass ------------------------------------------------

use lumen_static::desc::{NodeDesc, NO_PARENT};
use lumen_static::event::{EventCtx, Handlers, NodeSel};
use lumen_static::widget::Node;
use lumen_static::{Constraints, LayoutCx, NodeState, Size, Style};
use lumen_static_widgets::ops::Label;

/// A test-only leaf that watches the view: 20 x 20, flagged
/// `WATCHES_VIEW`, recording each `Event::Visibility` it is offered.
/// Its handler answers `NodeSel::visibility` with `on_view` and an
/// ordinary activation with `on_click` — never the other way round.
struct Watched<S: 'static> {
    on_view: Option<fn(&mut S, &EventCtx)>,
    on_click: Option<fn(&mut S, &EventCtx)>,
    heard: Vec<bool>,
}

impl<S: 'static> Default for Watched<S> {
    fn default() -> Self {
        Watched { on_view: None, on_click: None, heard: Vec::new() }
    }
}

fn watched_defaults() -> Style {
    Style { width: Length::Fixed(20.0), height: Length::Fixed(20.0), ..Style::default() }
}

impl<S: 'static> Widget for Watched<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::WATCHES_VIEW)];
    const KIND: &'static str = "watched";
    const SEEDS: &'static [lumen_static::style::Seed] =
        &[lumen_static::style::Seed::unlinted("watched", watched_defaults)];

    fn type_defaults() -> Style {
        watched_defaults()
    }
}

impl<S: 'static> Node for Watched<S> {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let s = lumen_static::ops::resolve_self(limits, &style[i], Size { w: 20.0, h: 20.0 });
        lumen_static::ops::record_painted(st, i, limits, s, &style[i])
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn lumen_static::Painter) {
        out.rect(lumen_static::ops::rect_of(&st[i]), lumen_static::display::Color(0x20, 0x60, 0xd0, 0xff));
    }

    fn on_event(&mut self, ev: &Event, _cx: &EventCtx) -> bool {
        if let Event::Visibility { visible } = *ev {
            self.heard.push(visible);
            return true;
        }
        false
    }

    lumen_static::leaf_common!(Watched<S>, NodeDesc::WATCHES_VIEW);
}

// ANCHOR: visibility_handler
impl<S: 'static> Handlers<S> for Watched<S> {
    fn handler(&self, sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        // Only the host's visibility question carries this sentinel; no
        // click ever does, so a click cannot run `on_view`.
        if sel.is_visibility() {
            return self.on_view;
        }
        if sel.inner.is_none() {
            return self.on_click;
        }
        None
    }
}
// ANCHOR_END: visibility_handler

// The unsplit application: its visibility handler writes a sibling label
// outside the leaf, which only a whole-tree mark redraws under
// `Boundaries<256>` (A11).

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct VContent {
    #[widget_id("filler")]
    filler: Spacer,
    #[widget_id("target")]
    target: Watched<VApp>,
    #[widget_id("tail")]
    tail: Spacer,
}

/// The label in a container of its own, as an application's readout
/// is. **Not a bare leaf beside the scroller**: a leaf is laid out
/// whenever its parent is, and the root is dirty whenever the leaf below
/// it is marked, so a bare label was re-laid on every frame and a
/// `ScrollEnd`-style mark left this gate green. A clean sibling
/// container is what `Boundaries` skips.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct VReadout {
    #[widget_id("readout")]
    text: Label<String>,
    #[widget_id("after")]
    after: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct VUi {
    #[widget_id("scroll")]
    scroll: ScrollView<VContent>,
    readout: VReadout,
}

#[derive(DeriveApp, Default)]
struct VApp {
    #[tree]
    ui: VUi,
    trues: u32,
    falses: u32,
    clicks: u32,
    /// Every run of the visibility handler, whatever event it was handed.
    view_calls: u32,
}

fn v_on_view(a: &mut VApp, ctx: &EventCtx) {
    a.view_calls += 1;
    if let Event::Visibility { visible } = ctx.event {
        if visible {
            a.trues += 1;
        } else {
            a.falses += 1;
        }
        a.ui.readout.text.set(format!("seen {}", a.trues));
    }
}

fn v_on_click(a: &mut VApp, _: &EventCtx) {
    a.clicks += 1;
}

fn v_app() -> VApp {
    let mut a = VApp::default();
    a.ui.scroll.content.target.on_view = Some(v_on_view);
    a.ui.scroll.content.target.on_click = Some(v_on_click);
    a.ui.readout.after.set("after".into());
    a
}

// The split application: the handler gets only the state, so the
// delivering frame marks what consumed the event and nothing else.

#[derive(Default)]
struct Seen {
    trues: u32,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct SContent {
    #[widget_id("filler")]
    filler: Spacer,
    #[widget_id("target")]
    target: Watched<Seen>,
    #[widget_id("tail")]
    tail: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct SUi {
    #[widget_id("scroll")]
    scroll: ScrollView<SContent>,
}

#[derive(DeriveApp, Default)]
struct SApp {
    #[tree]
    ui: SUi,
    #[state]
    seen: Seen,
}

fn s_on_view(s: &mut Seen, ctx: &EventCtx) {
    if let Event::Visibility { visible: true } = ctx.event {
        s.trues += 1;
    }
}

fn idx_of<T: Widget>(wid: &str) -> usize {
    lumen_static::style::index_of_wid(T::SUB_TABLE, wid).expect(wid)
}

fn rect_at(st: &[NodeState], i: usize) -> Rect {
    let s = st[i];
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

/// The modelled window loop: pump only while the host's deadline is due.
/// Answers how many frames it pumped.
fn pump_while_due<A: lumen_static::event::App, I: Invalidation>(h: &mut Host<A, Incremental, I>) -> u32 {
    let mut frames = 0;
    while h.next_deadline_ms().is_some_and(|d| d <= 0.0) {
        h.pump();
        frames += 1;
        assert!(frames < 10, "the deadline stayed due");
    }
    frames
}

/// One wheel step of the visibility walk.
#[derive(Debug)]
struct VStep {
    offset: f32,
    in_window: bool,
    in_viewport: bool,
    /// After the wheel's own pump, before the modelled loop.
    pending: usize,
    deadline: Option<f32>,
    /// Frames the modelled loop pumped.
    frames: u32,
    trues: u32,
    falses: u32,
    readout: String,
    hash: u64,
    /// The frame's damage after the loop, as the host reports it.
    damage: String,
}

fn v_host<I: Invalidation>() -> Host<VApp, Incremental, I> {
    let mut h: Host<VApp, Incremental, I> = Host::new(v_app(), WINDOW.0, WINDOW.1);
    h.set_stylesheet(&sheet());
    h.pump();
    pump_while_due(&mut h);
    h
}

/// Wheel `dy` at a time until the scroller stops, the modelled loop after
/// each wheel.
fn v_walk<I: Invalidation>(h: &mut Host<VApp, Incremental, I>, dy: f32) -> Vec<VStep> {
    let viewport = rect_at(&h.states, idx_of::<VUi>("scroll"));
    let window = [0.0, 0.0, WINDOW.0 as f32, WINDOW.1 as f32];
    let (x, y) = ((viewport[0] + viewport[2]) / 2.0, (viewport[1] + viewport[3]) / 2.0);
    let target = idx_of::<VUi>("target");
    let mut steps = Vec::new();
    loop {
        let before = h.app.ui.scroll.scroll;
        h.post(Event::Scroll { x, y, dy });
        let handled = h.pump();
        let offset = h.app.ui.scroll.scroll;
        if offset == before {
            break;
        }
        assert!(handled, "step {}: the wheel was not handled", steps.len() + 1);
        let r = rect_at(&h.states, target);
        let (pending, deadline) = (h.views_pending(), h.next_deadline_ms());
        let frames = pump_while_due(h);
        steps.push(VStep {
            offset,
            in_window: meets(r, window),
            in_viewport: meets(r, viewport),
            pending,
            deadline,
            frames,
            trues: h.app.trues,
            falses: h.app.falses,
            readout: h.app.ui.readout.text.as_str().to_string(),
            hash: h.list.hash_from(0),
            damage: format!("{:?}", h.damage()),
        });
        assert!(steps.len() < 100, "the scroller never stopped");
    }
    steps
}

fn print_steps(name: &str, dir: &str, steps: &[VStep]) {
    for (k, s) in steps.iter().enumerate() {
        println!(
            "  {name} {dir} step {:>2}: offset {:>5} in window {:<5} in viewport {:<5} pending {} deadline {:?} \
             loop frames {} true {} false {} readout {:?}",
            k + 1,
            s.offset,
            s.in_window,
            s.in_viewport,
            s.pending,
            s.deadline,
            s.frames,
            s.trues,
            s.falses,
            s.readout
        );
    }
}

/// The step (1-based) at which `pred` first holds.
fn first(steps: &[VStep], pred: impl Fn(&VStep) -> bool) -> Option<usize> {
    steps.iter().position(pred).map(|k| k + 1)
}

fn visibility<I: Invalidation>(name: &str) {
    let mut h: Host<VApp, Incremental, I> = v_host();
    println!("{name}: watched views {}", h.watched_views());
    assert_eq!(h.watched_views(), 1, "setup: the flagged leaf is not watched");
    assert_eq!((h.app.trues, h.app.falses), (0, 0), "setup: the leaf below the fold heard something");

    // In ...
    let down = v_walk(&mut h, -WHEEL);
    print_steps(name, "down", &down);
    assert!(!down.is_empty(), "setup: no wheel step moved the scroller");
    let gap = first(&down, |s| s.in_window && !s.in_viewport);
    let viewport_step = first(&down, |s| s.in_viewport);
    let arrival = first(&down, |s| s.trues == 1);
    println!(
        "  {name}: first inside the window and outside the viewport {gap:?}; first in the viewport \
         {viewport_step:?}; `true` arrived at {arrival:?}"
    );
    assert!(gap.is_some() && viewport_step.is_some(), "setup: T85.0's steps did not happen");
    let arrived = arrival.expect("`true` never arrived");
    // While a change waits, the deadline is due (G17: a loop that pumps
    // every step drains it whatever the deadline says, so it is asserted
    // itself).
    for (k, s) in down.iter().enumerate() {
        if s.pending > 0 {
            assert_eq!(s.deadline, Some(0.0), "step {}: {} change(s) pending and the deadline is {:?}", k + 1, s.pending, s.deadline);
        }
    }
    // The handler's write is drawn within one frame of the frame that
    // placed the leaf in view, by a loop that pumps only when due.
    let at = &down[arrived - 1];
    let shown = at.readout.as_str() == "seen 1";
    println!(
        "  {name}: frames from placement to the write, in the modelled loop: {} (bound 1), readout shown {shown}",
        at.frames
    );
    assert!(
        shown && at.frames == 1,
        "the visibility write was not drawn within one frame of placement ({} frames in the modelled loop, shown {shown})",
        at.frames
    );
    assert_eq!(
        arrival, viewport_step,
        "`true` did not arrive at the first step whose rect meets the viewport \
         (inside-window-outside-viewport step {gap:?})"
    );
    assert_eq!(h.app.trues, 1, "more than one `true` on the way in");

    // ... still ...
    let (t0, f0) = (h.app.trues, h.app.falses);
    let heard0 = h.app.ui.scroll.content.target.heard.len();
    for _ in 0..50 {
        h.pump();
    }
    let moved = (h.app.trues - t0) + (h.app.falses - f0) + (h.app.ui.scroll.content.target.heard.len() - heard0) as u32;
    println!("  {name}: 50 still frames delivered {moved} (bound 0)");
    assert_eq!(moved, 0, "still frames delivered visibility");

    // ... a click on it is a click ...
    let target = idx_of::<VUi>("target");
    let r = rect_at(&h.states, target);
    let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    let hit = h.probe(cx, cy).map(|x| x.node);
    let calls = h.app.view_calls;
    h.click(cx, cy);
    let handled = h.pump();
    println!(
        "  {name}: click at ({cx}, {cy}): hit {hit:?} (target {target}), handled {handled}, clicks {}, \
         visibility handler runs {calls} -> {} (bound unchanged)",
        h.app.clicks, h.app.view_calls
    );
    assert_eq!(hit, Some(target as u32), "setup: the click did not land on the leaf");
    assert_eq!(h.app.view_calls, calls, "a click ran the visibility handler");
    assert!(handled && h.app.clicks == 1, "the click was not handled by the leaf it hit");

    // ... and out.
    let up = v_walk(&mut h, WHEEL);
    print_steps(name, "up", &up);
    let left = first(&up, |s| !s.in_viewport);
    let departure = first(&up, |s| s.falses == 1);
    println!("  {name}: first step out of the viewport on the way up {left:?}; `false` arrived at {departure:?}");
    assert_eq!(departure, left, "`false` did not arrive at the first step out of the viewport");
    println!("  {name}: true {} false {} (bound 1 / 1); the leaf heard {:?}", h.app.trues, h.app.falses, h.app.ui.scroll.content.target.heard);
    assert_eq!((h.app.trues, h.app.falses), (1, 1), "in and out did not deliver exactly one true and one false");
    assert_eq!(h.app.ui.scroll.content.target.heard, vec![true, false], "the leaf was not offered exactly true, false");
}

#[test]
fn wheeling_a_watched_leaf_in_and_out_delivers_one_true_and_one_false() {
    visibility::<Everything>("Everything");
    visibility::<Boundaries<256>>("Boundaries<256>");
}

/// An unsplit application's handler writes a label outside the leaf: under
/// `Boundaries<256>` the next frame draws it, exactly as `Everything` does.
#[test]
fn an_unsplit_handlers_write_outside_the_leaf_is_drawn_under_boundaries() {
    let mut e: Host<VApp, Incremental, Everything> = v_host();
    let mut b: Host<VApp, Incremental, Boundaries<256>> = v_host();
    let (de, db) = (v_walk(&mut e, -WHEEL), v_walk(&mut b, -WHEEL));
    assert_eq!(de.len(), db.len(), "setup: the walks differ in length");
    let at = first(&de, |s| s.trues == 1).expect("setup: `true` never arrived");
    let (se, sb) = (&de[at - 1], &db[at - 1]);
    let prev = de[at - 2].hash;
    println!(
        "step {at}: readout {:?} / {:?}; Everything hash {:#x} (before {prev:#x}), Boundaries {:#x}; damage {:?}",
        se.readout,
        sb.readout,
        se.hash,
        sb.hash,
        sb.damage
    );
    assert_eq!(sb.readout, "seen 1", "setup: the handler did not write the label");
    assert_ne!(se.hash, prev, "setup: the write changed nothing in the Everything frame");
    for (k, (x, y)) in de.iter().zip(&db).enumerate() {
        assert_eq!(x.hash, y.hash, "step {}: the label written by the visibility handler is stale under Boundaries", k + 1);
    }
}

/// A split application's delivering frame marks what consumed the event,
/// not the whole tree.
#[test]
fn a_split_applications_visibility_frame_marks_only_what_consumed_it() {
    let mut a = SApp::default();
    a.ui.scroll.content.target.on_view = Some(s_on_view);
    let mut h: Host<SApp, Incremental, Boundaries<256>> = Host::new(a, WINDOW.0, WINDOW.1);
    h.set_stylesheet(&sheet());
    h.pump();
    pump_while_due(&mut h);
    let viewport = rect_at(&h.states, idx_of::<SUi>("scroll"));
    let (x, y) = ((viewport[0] + viewport[2]) / 2.0, (viewport[1] + viewport[3]) / 2.0);
    let mut marks = None;
    for _ in 0..40 {
        h.post(Event::Scroll { x, y, dy: -WHEEL });
        h.pump();
        if h.views_pending() > 0 {
            h.pump();
            marks = Some(h.last_marks());
            break;
        }
    }
    let marks = marks.expect("setup: the leaf never came into view");
    println!("split: the delivering frame marked {marks:?}; true {}", h.app.seen.trues);
    assert_eq!(h.app.seen.trues, 1, "setup: the split handler did not run");
    assert!(!marks.all && marks.ranges >= 1, "a split application's visibility frame marked {marks:?}");
}

/// `Host::adopt` while the leaf is in view: last frame's visibility
/// described the old application, so the new one hears `true`.
#[test]
fn an_adopted_application_hears_true_from_a_leaf_already_in_view() {
    for strategy in ["Everything", "Boundaries<256>"] {
        fn run<I: Invalidation>(name: &str) {
            let mut h: Host<VApp, Incremental, I> = v_host();
            v_walk(&mut h, -WHEEL);
            assert_eq!(h.app.trues, 1, "setup: the leaf is not in view");
            let mut fresh = v_app();
            fresh.ui.scroll.scroll = h.app.ui.scroll.scroll;
            h.adopt(fresh);
            let before = h.app.trues;
            h.pump();
            let after_one = h.app.trues;
            let frames = 1 + pump_while_due(&mut h);
            println!(
                "{name}: adopted app's count {before} -> {after_one} after one pump -> {} after {frames} \
                 (the pump that places it, then the one its deadline asks for)",
                h.app.trues
            );
            assert_eq!(before, 0, "setup: the adopted app is not fresh");
            assert_eq!(h.app.trues, 1, "the adopted application never heard `true` from a leaf in view");
            assert_eq!(frames, 2, "the adopted application heard `true` after {frames} frames (bound 2)");
        }
        match strategy {
            "Everything" => run::<Everything>(strategy),
            _ => run::<Boundaries<256>>(strategy),
        }
    }
}

#[test]
fn the_visibility_event_fits_sixteen_bytes() {
    let n = core::mem::size_of::<Event>();
    println!("size_of::<Event>() = {n} (bound 16)");
    assert_eq!(n, 16, "Event::Visibility grew Event past 16 B (K1)");
}
