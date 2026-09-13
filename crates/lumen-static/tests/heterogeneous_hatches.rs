//! `defaults()` must be a property of the type, not of the instance.
//!
//! The cascade runs when a stylesheet is set — twice in the life of a
//! `Host`, not per frame — and the arena containers snapshot each
//! template's defaults once. Anything state-dependent in `defaults()` is
//! therefore frozen at whatever the state was the first time it was
//! asked, and never revisited.
//!
//! `Option::defaults` used to answer `Style::default()` while `None`, so
//! every fixture in the suite that toggled one was silently broken and
//! none noticed: the existing hatch tests build homogeneous lists of
//! identical items, in which an instance-dependent answer is
//! unobservable. `docs/plan-review-fixes.md` F3.2.

use lumen_static::hatch::{Either, List, Side};
use lumen_static::ops::{Fixed, Spacer};
use lumen_static::widget::Node;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithOption {
    maybe: Option<Spacer>,
    tail: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithList {
    list: List<Option<Spacer>>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

/// Cascade with no rules — exactly what `Host::new` does — then lay out,
/// returning the computed styles so a caller can lay out *again* without
/// re-cascading. That is the whole point: the cascade runs twice in the
/// life of a `Host`, so a later frame reuses these.
fn frame<T: Widget + Node>(ui: &mut T, st: &mut [NodeState]) -> Vec<lumen_static::Style> {
    let mut styles = pass::default_styles(ui);
    lumen_static::style::cascade(
        T::SUB_TABLE,
        T::SEEDS,
        &[],
        &mut styles,
    );
    pass::layout(ui, &styles, st, VIEW);
    styles
}

#[test]
fn an_option_filled_after_the_cascade_still_fills() {
    // Constructed empty — the state the old `defaults()` froze.
    let mut ui = WithOption { maybe: None, tail: Fixed { w: 10.0, h: 10.0 } };
    let mut st = [NodeState::default(); WithOption::NODE_COUNT];
    let styles = frame(&mut ui, &mut st);
    // No main-axis extent, so it consumes none of the column and draws
    // nothing. Its *cross* extent is the container's, because the slot's
    // style is now the inner kind's (Fill) and `place` stretches
    // cross-fill children — absence is expressed by `layout` zeroing the
    // subtree, not by the style array, which the cascade owns. Zero area
    // is what "no space" means here.
    assert_eq!(st[1].size.h, 0.0, "absent: no main-axis extent");

    // Filled later, with no re-cascade — what an app toggling a panel
    // does. A `Spacer`'s defaults are Fill x Fill.
    // Reusing the styles the cascade already produced — no re-cascade,
    // which is what a `Host` actually does between frames.
    ui.maybe = Some(Spacer);
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    assert!(
        st[1].size.w > 0.0 && st[1].size.h > 0.0,
        "present: fills, rather than being stuck at the styling it had while empty ({:?})",
        st[1].size
    );
}

#[test]
fn a_list_is_not_styled_by_whichever_item_happened_to_be_first() {
    // items[0] is the probe `List::ensure` snapshots from. If its
    // defaults depended on its state, every other item would inherit
    // that state's styling.
    let mut ui = WithList {
        list: List::new(vec![None, Some(Spacer), Some(Spacer)]),
    };
    let mut st = [NodeState::default(); WithList::NODE_COUNT];
    frame(&mut ui, &mut st);

    let present = ui.list.item_state(1, 0).size;
    assert!(
        present.w > 0.0 && present.h > 0.0,
        "item 1 is Some and should fill, whatever item 0 is ({present:?})"
    );
    assert_eq!(
        ui.list.item_state(0, 0).size,
        lumen_static::Size::default(),
        "item 0 is None and takes no space"
    );
}

#[test]
fn either_reports_one_kinds_defaults_whichever_branch_shows() {
    // Both branches share one style slot, so `defaults()` has to name a
    // kind rather than a state. Toggling must not change the answer —
    // that is what makes the cascade's snapshot sound.
    let mut e: Either<Spacer, Fixed> = Either::new(Spacer, Fixed { w: 4.0, h: 4.0 });
    let a = e.defaults();
    e.show(Side::B);
    assert_eq!(e.defaults(), a, "toggling a branch does not restyle the slot");
    e.show(Side::A);
    assert_eq!(e.defaults(), a);
}
