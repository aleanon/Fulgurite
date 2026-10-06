//! **The visibility pass's setup** — M85/T85.0, the half that compiles
//! today (`docs/plan-layout-input.md`; T85.3 adds the pass and its gates
//! here).
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
//! (T85.0).

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
