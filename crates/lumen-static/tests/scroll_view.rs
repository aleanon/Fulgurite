//! M8: the scroll region — A1's containment, A5's constraints, A6's
//! clamp, A7's forbidden nesting.

use lumen_static::hatch::Either;
use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::repeat::{noop_bind, Repeat};
use lumen_static::scroll::ScrollView;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::widget::Node;
use lumen_static::{pass, Cmd, Constraints, DisplayList, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };

/// Four fixed blocks, 60px each: 240px of content in a 100px viewport.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Tall {
    a: Fixed,
    b: Fixed,
    c: Fixed,
    d: Fixed,
}

/// A column with a `Fill` spacer in it — the A5 NaN repro.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithFill {
    #[widget_id("gap")]
    gap: Spacer,
    #[widget_id("tail")]
    tail: Label,
}

fn tall() -> ScrollView<Tall> {
    let mut s = ScrollView::new(Tall::default());
    for f in [&mut s.content.a, &mut s.content.b, &mut s.content.c, &mut s.content.d] {
        f.w = 50.0;
        f.h = 60.0;
    }
    s
}

fn laid<W: Widget + Node>(w: &mut W, sheet: &[Rule]) -> Vec<NodeState> {
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    let mut sty = pass::default_styles(w);
    if !sheet.is_empty() {
        lumen_static::style::cascade_deep(w, sheet, &mut sty);
    }
    pass::layout(w, &sty, &mut st, VIEW);
    st
}

#[test]
fn content_may_exceed_the_viewport_and_the_scroller_still_fits_it() {
    let mut s = tall();
    let st = laid(&mut s, &[]);
    assert_eq!(st[0].size.h, 100.0, "the scroller fills the viewport, not the content");
    assert_eq!(st[1].size.h, 240.0, "the content measures its natural height");
}

#[test]
fn a_fill_child_inside_a_scroller_resolves_against_the_viewport_and_never_nans() {
    // A5's repro: `ScrollView { Column { Spacer /* Fill */, Label } }`.
    // The tempting implementation loosens the main axis to infinity, and
    // then `free = (inf - inf).max(0.0)` is NaN.
    let mut s = ScrollView::new(WithFill::default());
    s.content.tail.set("tail");
    let st = laid(&mut s, &[Rule::new(Selector::Id("gap"), Patch::new().height(Length::FILL))]);

    for (i, n) in st.iter().enumerate() {
        for v in [n.size.w, n.size.h, n.pos.x, n.pos.y] {
            assert!(v.is_finite(), "node {i} has a non-finite number: {n:?}");
        }
    }
    let gap = st[2];
    assert!(gap.size.h <= 100.0, "Fill means the viewport, not the content: {}", gap.size.h);
}

#[test]
fn the_offset_is_clamped_to_the_overflow_after_measuring() {
    // A6: content extent is known only after layout, so `layout` clamps.
    // A wheel handler may write anything.
    let mut s = tall();
    s.scroll = 1e9;
    let st = laid(&mut s, &[]);
    assert_eq!(s.scroll, 140.0, "240 of content in a 100 viewport overflows by 140");
    assert_eq!(s.overflow(&st, 0), 140.0);

    s.scroll = -50.0;
    laid(&mut s, &[]);
    assert_eq!(s.scroll, 0.0, "and cannot go above the top");

    // A non-finite offset is not clamped into a non-finite one.
    s.scroll = f32::NAN;
    laid(&mut s, &[]);
    assert_eq!(s.scroll, 0.0, "NaN is reset, not propagated");
}

#[test]
fn content_that_does_not_overflow_cannot_be_scrolled() {
    let mut s = ScrollView::new(Tall::default());
    s.content.a.h = 10.0;
    s.scroll = 500.0;
    laid(&mut s, &[]);
    assert_eq!(s.scroll, 0.0, "nothing to scroll");
}

#[test]
fn paint_is_clipped_to_the_viewport_and_hit_agrees_with_it() {
    let mut s = tall();
    s.scroll = 0.0;
    let mut st = vec![NodeState::default(); <ScrollView<Tall> as Widget>::NODE_COUNT];
    let sty = pass::default_styles(&s);
    pass::layout(&mut s, &sty, &mut st, VIEW);

    let mut list = DisplayList::new();
    pass::paint(&s, &st, &mut list);
    let clips = list.cmds.iter().filter(|c| matches!(c, Cmd::PushClip { .. })).count();
    assert!(clips >= 1, "the scroller opens a clip scope");
    let Some(Cmd::PushClip { rect }) = list.cmds.iter().find(|c| matches!(c, Cmd::PushClip { .. }))
    else {
        panic!("no clip")
    };
    assert_eq!(rect[3] - rect[1], 100.0, "the scope is the viewport, not the content");

    // The fourth block sits at y=180, well past the viewport: painted
    // (and clipped away), never hit.
    assert!(pass::hit_test(&s, &st, 10.0, 190.0).is_none(), "outside the viewport is not hit");
    assert!(pass::hit_test(&s, &st, 10.0, 50.0).is_some(), "inside it is");
}

#[test]
fn scrolling_moves_the_content_and_the_hit_target_together() {
    let mut s = tall();
    let mut st = vec![NodeState::default(); <ScrollView<Tall> as Widget>::NODE_COUNT];
    let sty = pass::default_styles(&s);
    pass::layout(&mut s, &sty, &mut st, VIEW);
    let before = pass::hit_test(&s, &st, 10.0, 70.0).map(|h| h.node);

    s.scroll = 60.0;
    pass::layout(&mut s, &sty, &mut st, VIEW);
    let after = pass::hit_test(&s, &st, 10.0, 10.0).map(|h| h.node);
    assert_eq!(before, after, "the row that was at y=70 is at y=10 after scrolling 60");
    assert_eq!(st[1].pos.y, -60.0, "the content is placed at the offset");
}

#[test]
fn a_scroller_is_rigid_so_its_content_cannot_resize_the_page() {
    // The whole point: a scroller's size comes from its parent. If it
    // were content-sized it would grow instead of scrolling.
    const _: () = assert!(<ScrollView<Tall> as Widget>::IS_RIGID);
    assert_eq!(<ScrollView<Tall> as Widget>::NODE_COUNT, 1 + Tall::NODE_COUNT);
    assert!(lumen_static::widget::table_is_well_formed(<ScrollView<Tall> as Widget>::SUB_TABLE));
}

#[test]
fn a_repeat_reports_itself_as_a_virtualizer() {
    // What A7's `W1008` lint keys on. Asserted here rather than only in
    // the agent crate so the signal cannot be removed from the core
    // without a core test failing.
    let r = Repeat::<Tall>::new(10, 24.0, noop_bind);
    assert!(r.virtualizes(), "a Repeat windows its content");
    assert!(!Tall::default().virtualizes(), "an ordinary container does not");
    assert!(!tall().virtualizes(), "and neither does a scroller");
}

#[test]
fn a_scroller_inside_an_either_branch_works() {
    // The composition T8.0 was sequenced before this milestone for.
    type Tabs = Either<ScrollView<Tall>, Label>;
    let mut e: Tabs = Either::new(tall(), Label::new("other"));
    let mut st = vec![NodeState::default(); <Tabs as Widget>::NODE_COUNT];
    let sty = pass::default_styles(&e);
    pass::layout(&mut e, &sty, &mut st, VIEW);

    // The scroller is branch A's root, at index 1.
    assert_eq!(st[1].size.h, 100.0, "the scroller fills the viewport inside the branch");
    assert_eq!(st[2].size.h, 240.0, "and its content still overflows");
}

#[test]
fn the_repeaters_f32_scroll_ceiling_has_a_number() {
    // Deferred twice without one (`docs/step3-repeaters.md`). `f32`
    // holds consecutive integers exactly to 2^24; past that, one ulp of
    // `scroll` exceeds a row and rows become unreachable.
    let r = Repeat::<Tall>::new(0, 24.0, noop_bind);
    assert_eq!(r.max_addressable_rows(), 699_050, "24 px rows");

    let fine = Repeat::<Tall>::new(0, 1.0, noop_bind);
    assert_eq!(fine.max_addressable_rows(), 16_777_216, "1 px rows");

    // The ceiling is a property of the extent, and a degenerate extent
    // addresses nothing rather than dividing by it.
    let bad = Repeat::<Tall>::new(0, 0.0, noop_bind);
    assert_eq!(bad.max_addressable_rows(), 0);

    // At the ceiling, the last row is still selectable: one ulp of the
    // offset is no wider than a row.
    let extent = 24.0f32;
    let last = (699_050 - 1) as f32 * extent;
    assert!(
        (last + extent) - last >= extent,
        "an offset one row past {last} must be representable"
    );
}

// ---- C3/C4: the scroller consumes the wheel, and chains -------------

use lumen_static::event::Event;

#[derive(lumen_static::App, Default)]
struct Page {
    #[tree]
    ui: ScrollView<Tall>,
}

fn page() -> Page {
    let mut p = Page::default();
    for f in [&mut p.ui.content.a, &mut p.ui.content.b, &mut p.ui.content.c, &mut p.ui.content.d] {
        f.w = 50.0;
        f.h = 60.0;
    }
    p
}

fn wheel(p: &mut Page, st: &mut [NodeState], dy: f32) -> bool {
    let sty = pass::default_styles(p.ui_ref());
    pass::layout(&mut p.ui, &sty, st, VIEW);
    let mut input = lumen_static::event::InputState::default();
    pass::dispatch(p, &sty, st, &mut input, &Event::Scroll { x: 10.0, y: 10.0, dy })
}

impl Page {
    fn ui_ref(&self) -> &ScrollView<Tall> {
        &self.ui
    }
}

#[test]
fn a_wheel_event_scrolls_the_region_it_lands_in() {
    // C3: before this, `input.scroll` was enqueued, drained and dropped
    // — the event vocabulary existed and nothing consumed it.
    let mut p = page();
    let mut st = vec![NodeState::default(); <ScrollView<Tall> as Widget>::NODE_COUNT];
    assert!(wheel(&mut p, &mut st, -20.0), "the scroller consumed the wheel");
    assert_eq!(p.ui.scroll, 20.0);
    assert!(wheel(&mut p, &mut st, -20.0));
    assert_eq!(p.ui.scroll, 40.0);
}

#[test]
fn a_scroller_at_its_end_declines_so_a_parent_can_chain() {
    // C4. The `bool` from `on_event` is the whole mechanism: at the end
    // of travel the scroller returns false and `dispatch` keeps walking
    // up. Without it, an inner list at its bottom would swallow the rest
    // of the gesture and the page would not move.
    let mut p = page();
    let mut st = vec![NodeState::default(); <ScrollView<Tall> as Widget>::NODE_COUNT];

    assert!(!wheel(&mut p, &mut st, 20.0), "already at the top: declined");
    assert_eq!(p.ui.scroll, 0.0);

    // Drive to the bottom, then past it.
    for _ in 0..10 {
        wheel(&mut p, &mut st, -50.0);
    }
    assert_eq!(p.ui.scroll, 140.0, "clamped to the overflow");
    assert!(!wheel(&mut p, &mut st, -50.0), "at the bottom: declined, so a parent gets it");
}

#[test]
fn a_wheel_over_content_that_fits_is_declined() {
    let mut p = Page::default();
    p.ui.content.a.h = 10.0;
    let mut st = vec![NodeState::default(); <ScrollView<Tall> as Widget>::NODE_COUNT];
    assert!(!wheel(&mut p, &mut st, -50.0), "nothing to scroll, nothing consumed");
}
