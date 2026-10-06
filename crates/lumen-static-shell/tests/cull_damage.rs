//! **What culling costs retention** — M85/T85.6's pin, the gap the
//! plan's X1 names (`docs/plan-layout-input.md`, Decisions, "The
//! retention it breaks").
//!
//! A `List` culls its items against `Painter::clip_bounds` (T16.12), so
//! as it scrolls the number of commands it paints changes. `Incremental`
//! compares two frames slot for slot when their lengths agree and, when
//! they do not, by their longest common prefix and suffix (M26/T26.11):
//! everything between the first and the last difference is damage. So a
//! culled range that changes its count while anything *after* it also
//! changes charges every unchanged command between the two — here a label
//! beside the scroller, painted after it, with a readout after that which
//! the test writes every step. With culling off (`bounds` `None`) the
//! count never changes, the frames compare slot for slot, and the label
//! is never damage.
//!
//! The tree: a `List` of 220 one-row `List`s (each row clips to itself,
//! so a row's own clip pops at a deeper depth than the list's) in a
//! 240 × 200 `ScrollView`, a label and a readout to its right, in a
//! 400 × 400 window; ten wheel steps of 7 px under
//! `Host<_, Incremental, Everything>` and `Host<_, Incremental,
//! Boundaries<256>>`. Each step prints the damage in pixels inside the
//! scroller's rect, outside it, and over the label, for both arms, beside
//! the list's command count.
//!
//! **The `None` arm is a mirror, not a second host.** `Host::frame`
//! always sets the surface as the bound, so after each pump the test
//! repaints the host's laid-out tree through `Scaled(Recording(list))`, the
//! host's order, into its own
//! `Incremental` twice — once with `set_bounds(None)`, once with the
//! surface — and asserts the surface mirror's damage equals the host's at
//! every step, which is what makes the `None` mirror the host's frame
//! with culling off and nothing else changed.
//!
//! **Pinned green at T85.6** (index §3.1, no `#[ignore]`): the label is
//! damage on the steps whose count changed, and the damage outside the
//! scroller is larger with culling than without. T85.7 built the fix that
//! flips it (scope ends, and `Incremental` re-synchronising at them), and
//! K85c reverted it on its per-scope cost, so the pin stands as the
//! record of what `List`'s cull costs retention.
//! The figures are recorded once, in `docs/results-layout-input.md`
//! (T85.6).

use lumen_static::hatch::List;
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::retain::{Damage, Incremental, Retention};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, DisplayList, Event, Recording, Scaled, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static_widgets::ScrollView;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Body {
    #[widget_id("list")]
    list: List<List<Fixed>>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Side {
    #[widget_id("label")]
    label: Label<&'static str>,
    #[widget_id("readout")]
    readout: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Ui {
    #[widget_id("scroll")]
    scroll: ScrollView<Body>,
    side: Side,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const WINDOW: (u32, u32) = (400, 400);
const ROWS: usize = 220;
const ROW: (f32, f32) = (200.0, 24.0);
const WHEEL: f32 = 7.0;
const STEPS: usize = 10;

/// `[x0, y0, x1, y1]`.
type Rect = [f32; 4];

fn rect_of<I: Invalidation>(h: &Host<App, Incremental, I>, wid: &str) -> Rect {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

/// Damaged pixels of the window, split into those whose centre is in
/// `inside` and the rest. Regions may overlap; a pixel counts once.
fn pixels(d: Damage, inside: Rect) -> (usize, usize) {
    let (w, h) = (WINDOW.0 as usize, WINDOW.1 as usize);
    let window = [0.0, 0.0, w as f32, h as f32];
    let mut mask = vec![false; w * h];
    for r in d.regions(window) {
        for y in (r[1].floor().max(0.0) as usize)..(r[3].ceil().min(h as f32) as usize) {
            for x in (r[0].floor().max(0.0) as usize)..(r[2].ceil().min(w as f32) as usize) {
                mask[y * w + x] = true;
            }
        }
    }
    let (mut a, mut b) = (0, 0);
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                if fx >= inside[0] && fx < inside[2] && fy >= inside[1] && fy < inside[3] {
                    a += 1;
                } else {
                    b += 1;
                }
            }
        }
    }
    (a, b)
}

/// One arm's damage at one step.
#[derive(Clone, Copy, Debug)]
struct Arm {
    cmds: usize,
    inside: usize,
    outside: usize,
    label: usize,
}

struct Step {
    handled: bool,
    offset: f32,
    culled: Arm,
    none: Arm,
}

/// Repaint the host's laid-out tree into `list` with `bounds`, through
/// `Recording` into `ret`, and close the frame.
fn mirror<I: Invalidation>(
    h: &Host<App, Incremental, I>,
    ret: &mut Incremental,
    list: &mut DisplayList,
    bounds: Option<Rect>,
) -> (Damage, usize) {
    list.clear();
    list.set_bounds(bounds);
    {
        // `Host::frame`'s order: `Scaled(Recording(list))`, which at a
        // scale of 1.0 still snaps every rect to the pixel grid.
        let mut rec = Recording::new(list, ret);
        let mut out = Scaled::new(&mut rec, 1.0);
        lumen_static::pass::paint(&h.app.ui, &h.states, &mut out);
    }
    (ret.finish(), list.cmds.len())
}

fn walk<I: Invalidation>(name: &str) -> Vec<Step> {
    let mut app = App::default();
    app.ui.side.label.set("after the list");
    app.ui.scroll.content.list =
        List::new((0..ROWS).map(|_| List::new(vec![Fixed { w: ROW.0, h: ROW.1 }])).collect());
    let mut h: Host<App, Incremental, I> = Host::new(app, WINDOW.0, WINDOW.1);
    h.set_stylesheet(&[Rule::new(
        Selector::Id("scroll"),
        Patch::new().width(Length::Fixed(240.0)).height(Length::Fixed(200.0)),
    )]);
    h.pump();
    h.take_damage_for_age(1);
    let sc = rect_of(&h, "scroll");
    let label = rect_of(&h, "label");
    println!("{name}: scroller {sc:?}, label {label:?}, window {WINDOW:?}");
    let surface = Some([0.0, 0.0, WINDOW.0 as f32, WINDOW.1 as f32]);
    let (mut ret_none, mut ret_some) = (Incremental::default(), Incremental::default());
    let mut list = DisplayList::new();
    mirror(&h, &mut ret_none, &mut list, None);
    mirror(&h, &mut ret_some, &mut list, surface);
    let (x, y) = ((sc[0] + sc[2]) / 2.0, (sc[1] + sc[3]) / 2.0);
    let mut steps = Vec::new();
    for k in 0..STEPS {
        // Something after the label changes every step, so a frame whose
        // length changed has a difference on each side of the label.
        h.app.ui.side.readout.set(format!("step {k}"));
        h.invalidate_all();
        h.post(Event::Scroll { x, y, dy: -WHEEL });
        let handled = h.pump();
        // Everything since the last present: a pump may paint more than
        // once, and the mirror paints once per step.
        let d = h.take_damage_for_age(1);
        let (ds, ns) = mirror(&h, &mut ret_some, &mut list, surface);
        assert_eq!(
            (ds, ns),
            (d, h.list.cmds.len()),
            "{name} step {}: the surface mirror's damage and count are not the host's -- the None mirror would not be the host's frame",
            k + 1
        );
        let (dn, nn) = mirror(&h, &mut ret_none, &mut list, None);
        let arm = |d: Damage, cmds: usize| {
            let (inside, outside) = pixels(d, sc);
            Arm { cmds, inside, outside, label: pixels(d, label).0 }
        };
        let s = Step { handled, offset: h.app.ui.scroll.scroll, culled: arm(d, ns), none: arm(dn, nn) };
        println!(
            "  step {:>2}: handled {} offset {:>3} | culled: cmds {:>3}, px inside {:>5} outside {:>5} label {:>4} | none: cmds {:>3}, px inside {:>5} outside {:>5} label {:>4}",
            k + 1,
            s.handled,
            s.offset,
            s.culled.cmds,
            s.culled.inside,
            s.culled.outside,
            s.culled.label,
            s.none.cmds,
            s.none.inside,
            s.none.outside,
            s.none.label
        );
        steps.push(s);
    }
    // Setup landed: every wheel handled and moved the scroller, the list
    // culled (its count changed between steps), and the uncull arm painted
    // every row every time.
    let mut last = 0.0;
    for (k, s) in steps.iter().enumerate() {
        assert!(s.handled, "{name} step {}: the wheel was not handled", k + 1);
        assert!(s.offset > last, "{name} step {}: the offset did not move ({} after {last})", k + 1, s.offset);
        last = s.offset;
        assert!(s.none.cmds > ROWS, "{name} step {}: the uncull arm painted {} commands for {ROWS} rows", k + 1, s.none.cmds);
    }
    let changed = steps.windows(2).filter(|w| w[0].culled.cmds != w[1].culled.cmds).count();
    println!("  steps whose culled count differs from the step before: {changed} of {}", STEPS - 1);
    assert!(changed > 0, "{name}: the list's command count never changed -- nothing culled, so nothing can shift");
    assert!(
        steps.iter().all(|s| s.none.cmds == steps[0].none.cmds),
        "{name}: the uncull arm's count changed, so it is not the arm with nothing shifted"
    );
    steps
}

/// T85.6's pin: today's gap, asserted green.
fn pin(name: &str, steps: &[Step]) {
    let culled: usize = steps.iter().map(|s| s.culled.outside).sum();
    let none: usize = steps.iter().map(|s| s.none.outside).sum();
    let label_culled: usize = steps.iter().map(|s| s.culled.label).sum();
    let label_none: usize = steps.iter().map(|s| s.none.label).sum();
    println!(
        "  {name}: damage outside the scroller over {STEPS} steps: culled {culled} px, none {none} px; over the label: culled {label_culled} px, none {label_none} px"
    );
    assert_eq!(label_none, 0, "{name}: the label was damage with culling off -- the uncull arm is not slot for slot");
    assert!(
        label_culled > 0,
        "{name}: culling no longer charges the label ({label_culled} px) -- the gap this pins has closed; flip the gate"
    );
    assert!(
        culled > none,
        "{name}: damage outside the scroller with culling ({culled} px) is not above without ({none} px) -- the gap this pins has closed; flip the gate"
    );
}

#[test]
fn a_culled_list_charges_what_follows_it_when_its_count_changes() {
    let e = walk::<Everything>("Everything");
    pin("Everything", &e);
    let b = walk::<Boundaries<256>>("Boundaries<256>");
    pin("Boundaries<256>", &b);
}
