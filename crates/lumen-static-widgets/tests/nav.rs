//! M27/T27.6 — the navigation group.
//!
//! The plan grouped these as "one shape five times". **Three of the five
//! are**; `AppBar` is not a selector and `PullToRefresh` is not a strip.
//! These tests are written per widget rather than per group for that
//! reason: a shared test would have had to pretend an app bar has a
//! selected index.

// on_event only: these test a strip's index arithmetic and a pull's
// thresholds. Delivery, hover and press shades included, is
// scripts/drive/widgets/nav.py against the live window under both
// strategies. (M57/D2)

use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::{AppBar, BottomNav, NavigationRail, PullToRefresh, Tabs};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };
const LABELS: [&str; 4] = ["Home", "Search", "Saved", "Profile"];

fn key(n: &mut dyn Node, k: Key) -> bool {
    let ev = Event::Key { key: k, down: true };
    n.on_event(&ev, &EventCtx::for_event(0, ev))
}

fn press_at<W: Widget + Node>(w: &mut W, local: Point) -> bool {
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = local;
    w.on_event(&ev, &cx)
}

fn lay<W: Widget + Node>(w: &mut W) -> Vec<NodeState> {
    let styles = vec![<W as Widget>::type_defaults(); <W as Widget>::NODE_COUNT];
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

// ---- the three that really are one shape ----------------------------

#[test]
fn a_press_selects_the_item_under_it() {
    let mut t = Tabs::new(&LABELS);
    let st = lay(&mut t);
    let w = st[0].size.w;
    assert!(w > 0.0, "setup: the strip has no width");
    // The third of four cells.
    assert!(press_at(&mut t, Point { x: w * 0.6, y: 10.0 }));
    assert_eq!(
        t.selected(), 2,
        "a press 60% along a four-item strip selected {} ({:?})",
        t.selected(), t.label(t.selected())
    );
}

/// The horizontal and the vertical kind read the *same* strip along
/// different axes — which is the claim "one shape, three presentations"
/// actually makes, and the one a shared implementation can get wrong.
#[test]
fn a_rail_reads_the_press_down_its_own_axis() {
    let mut r = NavigationRail::new(&LABELS);
    let st = lay(&mut r);
    let h = st[0].size.h;
    assert!(h > 0.0, "setup: the rail has no height");
    // Two and a half items *down*, with x near zero: a rail that read
    // `local.x` would land on item 0 and this is the only test that
    // could tell. (Items are 56 px from the top since M57/B38.)
    assert!(h > 56.0 * 4.0, "setup: the rail is shorter than its items");
    assert!(press_at(&mut r, Point { x: 4.0, y: 56.0 * 2.5 }));
    assert_eq!(r.selected(), 2, "a rail selected {} reading down its axis", r.selected());
}

#[test]
fn the_keyboard_moves_the_selection_and_stops_at_the_ends() {
    let mut t = BottomNav::new(&LABELS);
    lay(&mut t);
    assert_eq!(t.selected(), 0);
    key(&mut t, Key::Right);
    key(&mut t, Key::Right);
    assert_eq!(t.selected(), 2);
    for _ in 0..10 {
        key(&mut t, Key::Right);
    }
    assert_eq!(t.selected(), 3, "it ran past the last item: {}", t.selected());
    for _ in 0..10 {
        key(&mut t, Key::Left);
    }
    assert_eq!(t.selected(), 0, "it ran past the first item: {}", t.selected());
}

/// A press on the already-selected item is still **handled**. Declining
/// would let it bubble, and a scroller above would then take it as a
/// drag — which is the bug this arm exists to prevent, not a nicety.
#[test]
fn a_press_on_the_current_item_is_still_consumed() {
    let mut t = Tabs::new(&LABELS);
    let st = lay(&mut t);
    let w = st[0].size.w;
    press_at(&mut t, Point { x: w * 0.1, y: 10.0 });
    assert_eq!(t.selected(), 0);
    assert!(
        press_at(&mut t, Point { x: w * 0.1, y: 10.0 }),
        "a press on the selected item was declined and would bubble to a parent"
    );
}

#[test]
fn an_out_of_range_selection_is_clamped_not_ignored() {
    let mut t = Tabs::new(&LABELS);
    t.select(99);
    assert_eq!(
        t.selected(), 3,
        "a strip with an out-of-range selection shows nothing selected at all"
    );
}

#[test]
fn a_strip_truncates_rather_than_refusing() {
    let many: [&str; 12] = ["a"; 12];
    let t = Tabs::new(&many);
    assert_eq!(t.count(), lumen_static_widgets::MAX_ITEMS, "it did not truncate");
}

/// The three kinds answer three names at compile time — which is what
/// makes a `Kind` selector able to style a rail differently from tabs.
#[test]
fn the_three_kinds_have_three_names() {
    assert_eq!(<Tabs as Widget>::KIND, "tabs");
    assert_eq!(<BottomNav as Widget>::KIND, "bottomnav");
    assert_eq!(<NavigationRail as Widget>::KIND, "navigationrail");
    // And three shapes: a rail is tall and narrow, the others wide.
    let rail = <NavigationRail as Widget>::type_defaults();
    let tabs = <Tabs as Widget>::type_defaults();
    assert_ne!(
        format!("{:?}", rail.width), format!("{:?}", tabs.width),
        "a rail and a tab bar resolved the same width — one of them is wrong"
    );
}

// ---- AppBar: not a selector -----------------------------------------

/// **Rigid means rigid.** The bar's height is its style's, never its
/// title's — a longer title must not make it taller, or `IS_RIGID` is a
/// lie and a title change relays out the whole screen below it.
///
/// **Laid out under a `Shrink` height on purpose.** The first version of
/// this used the defaults, where `height: Fixed(56.0)` makes
/// `resolve_self` ignore the content outright — so an injection that
/// made `layout` measure the title left it green. The test proved the
/// *style*, not the code it is about. Under `Shrink` the natural height
/// is what decides, which is exactly where a measurement would show up.
#[test]
fn an_app_bar_does_not_grow_with_its_title() {
    use lumen_static::style::Length;
    // A `const` claim, checked at compile time — clippy asks for the
    // const block and is right: this cannot vary at runtime, and a
    // runtime `assert!` on it reads as though it could.
    const { assert!(<AppBar as Widget>::IS_RIGID, "an app bar claims not to be rigid") };
    let shrink = lumen_static::Style {
        height: Length::Shrink,
        ..<AppBar as Widget>::type_defaults()
    };
    let measure = |title: &'static str| {
        let mut bar: AppBar = AppBar::new(title);
        let styles = vec![shrink];
        let mut st = vec![NodeState::default()];
        let mut m = lumen_static::cx::BlockMetrics;
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut bar, &mut cx, VIEW, &styles, &mut st, 0);
        st[0].size.h
    };
    let (a, b) = (
        measure("Hi"),
        measure("A title long enough to wrap several times over in any sane measure"),
    );
    assert!(a > 0.0, "setup: the bar has no height at all under Shrink");
    assert_eq!(a, b, "the bar grew from {a} to {b} with a longer title");
}

// ---- PullToRefresh: not a strip -------------------------------------

#[test]
fn a_pull_past_the_threshold_arms_a_refresh_once() {
    let mut p: PullToRefresh<lumen_static_widgets::ops::Spacer> = PullToRefresh::default();
    lay(&mut p);
    // Half the wheel's distance pulls (M57/B40: the pull resists), so a
    // 160 px wheel is an 80 px pull.
    let ev = Event::Scroll { x: 0.0, y: 0.0, dy: 160.0 };
    assert!(p.on_event(&ev, &EventCtx::for_event(0, ev)), "the pull was not taken");
    assert!(p.is_armed(), "80 px of pull did not pass the 64 px threshold: {}", p.pull());
    let up = Event::PointerUp { x: 0.0, y: 0.0, button: lumen_static::PointerButton::Left };
    p.on_event(&up, &EventCtx::for_event(0, up));
    assert!(p.take_triggered(), "releasing an armed pull did not trigger");
    assert!(
        !p.take_triggered(),
        "the same gesture triggered twice — a double refresh is what `take` prevents"
    );
    assert_eq!(p.pull(), 0.0, "it did not spring back");
}

#[test]
fn a_short_pull_does_not_arm_anything() {
    let mut p: PullToRefresh<lumen_static_widgets::ops::Spacer> = PullToRefresh::default();
    lay(&mut p);
    let ev = Event::Scroll { x: 0.0, y: 0.0, dy: 10.0 };
    p.on_event(&ev, &EventCtx::for_event(0, ev));
    assert!(!p.is_armed(), "10 px armed a 64 px threshold");
    let up = Event::PointerUp { x: 0.0, y: 0.0, button: lumen_static::PointerButton::Left };
    p.on_event(&up, &EventCtx::for_event(0, up));
    assert!(!p.take_triggered(), "a short pull triggered a refresh");
}

/// **An upward wheel is the content's.** Taking it would mean a list
/// inside one could never be scrolled back up.
#[test]
fn an_upward_wheel_is_not_a_pull() {
    let mut p: PullToRefresh<lumen_static_widgets::ops::Spacer> = PullToRefresh::default();
    lay(&mut p);
    let ev = Event::Scroll { x: 0.0, y: 0.0, dy: -80.0 };
    assert!(
        !p.on_event(&ev, &EventCtx::for_event(0, ev)),
        "it consumed an upward wheel; nothing inside it could scroll back"
    );
    assert_eq!(p.pull(), 0.0);
}

/// The content moves with the pull, or the indicator reads as an overlay
/// rather than as a drag.
#[test]
fn the_content_follows_the_pull() {
    let mut p: PullToRefresh<lumen_static_widgets::ops::Spacer> = PullToRefresh::default();
    let before = lay(&mut p)[1].pos.y;
    // An 80 px wheel is a 40 px pull: the pull resists (M57/B40).
    let ev = Event::Scroll { x: 0.0, y: 0.0, dy: 80.0 };
    p.on_event(&ev, &EventCtx::for_event(0, ev));
    let after = lay(&mut p)[1].pos.y;
    assert_eq!(p.pull(), 40.0, "setup: an 80 px wheel is not a 40 px pull");
    assert_eq!(
        after - before, 40.0,
        "the content moved {} px for a 40 px pull",
        after - before
    );
}

/// **A filled strip must have an opaque ground**, because the selected
/// item's label is drawn *in* that ground — it sits on the foreground
/// fill. A transparent default makes the current item's label invisible:
/// dark text on a dark pill.
///
/// Found by rendering a `NavigationRail` and seeing "Home" vanish. No
/// test would have caught it: every assertion about the selection was
/// about geometry or about the index, and both were correct.
#[test]
fn a_filled_strip_has_a_ground_for_its_selected_label() {
    for (name, s) in [
        ("BottomNav", <BottomNav as Widget>::type_defaults()),
        ("NavigationRail", <NavigationRail as Widget>::type_defaults()),
    ] {
        assert!(
            s.background.3 > 0,
            "{name}'s default background is {:?} — its selected label is drawn in that \
             colour, so a transparent one is an invisible label on a filled pill",
            s.background
        );
        // And it must actually differ from the fill it sits on, or the
        // label is the same colour as the pill.
        assert_ne!(
            (s.background.0, s.background.1, s.background.2),
            (s.color.0, s.color.1, s.color.2),
            "{name}'s ground and its fill are the same colour"
        );
    }
}

// ---- M57/B36–B38: hover, the accent, a rail's items ---------------------

fn ev_at<W: Widget + Node>(w: &mut W, ev: Event, local: Point) -> bool {
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = local;
    w.on_event(&ev, &cx)
}

fn painted<W: Widget + Node>(w: &W, st: &[NodeState]) -> Vec<lumen_static::display::Cmd> {
    let mut list = lumen_static::display::DisplayList::new();
    Node::paint(w, st, 0, &mut list);
    list.cmds
}

/// A rail stacks 56 px items from its top: it divided its whole height
/// among them, so the items grew with the window and a press below the
/// last one selected it (`docs/widget-audit.md`, NavigationRail).
#[test]
fn a_rail_stacks_fixed_items_from_its_top() {
    let mut r = NavigationRail::new(&LABELS);
    let st = lay(&mut r);
    let h = st[0].size.h;
    println!("rail height {h}, a press at y=60 and at y={}", h - 10.0);
    assert!(h > 56.0 * 4.0 + 20.0, "setup: the rail has no room below its items");
    assert!(press_at(&mut r, Point { x: 4.0, y: 60.0 }));
    assert_eq!(r.selected(), 1, "y=60 is the second 56 px item");
    assert!(!press_at(&mut r, Point { x: 4.0, y: h - 10.0 }), "a press below the last item was taken");
    assert_eq!(r.selected(), 1, "a press below the last item changed the selection");
}

/// Hover and press are shown: paint ignored both (B36).
#[test]
fn a_hovered_item_is_tinted_and_a_pressed_one_more() {
    let mut t = Tabs::new(&LABELS);
    let mut st = lay(&mut t);
    let w = st[0].size.w;
    let fills = |cmds: &[lumen_static::display::Cmd]| -> Vec<u8> {
        cmds.iter()
            .filter_map(|c| match c {
                lumen_static::display::Cmd::RoundedRect { color, .. } if color.3 > 0 && color.3 < 0xff => Some(color.3),
                _ => None,
            })
            .collect()
    };
    let before = fills(&painted(&t, &st));
    assert!(ev_at(&mut t, Event::Hover { x: 0.0, y: 0.0 }, Point { x: w * 0.6, y: 10.0 }), "a hover over a new item reported no change");
    let hover = fills(&painted(&t, &st));
    st[0].pressed = true;
    let press = fills(&painted(&t, &st));
    println!("translucent fills: idle {before:?}, hovered {hover:?}, pressed {press:?}");
    assert!(before.is_empty(), "setup: an idle strip already paints a tint");
    assert_eq!(hover.len(), 1, "no tint under the pointer");
    assert!(press[0] > hover[0], "a press is not shown over the hover");
    assert!(ev_at(&mut t, Event::Leave, Point { x: 0.0, y: 0.0 }), "leaving reported no change");
    st[0].pressed = false;
    assert!(fills(&painted(&t, &st)).is_empty(), "the tint outlived the pointer");
}

/// The current tab is marked in the accent — its underline and its label —
/// where it was the text colour and only the weight told it apart (B37).
#[test]
fn the_current_tab_is_marked_in_the_accent() {
    let mut t = Tabs::new(&LABELS);
    let mut st = lay(&mut t);
    let accent = lumen_static::display::Color(0x25, 0x63, 0xeb, 0xff);
    st[0].border_color = accent;
    let cmds = painted(&t, &st);
    let under = cmds.iter().filter(|c| matches!(c, lumen_static::display::Cmd::Rect { color, .. } if *color == accent)).count();
    let label = cmds.iter().filter(|c| matches!(c, lumen_static::display::Cmd::Text { color, .. } if *color == accent)).count();
    println!("accent rects {under}, accent labels {label}");
    assert_eq!((under, label), (1, 1), "the current tab is not drawn in the accent");
    // And the palette hands Tabs an accent at all.
    let s = <Tabs as Widget>::SEEDS[0].style(&lumen_static::style::Palette::LIGHT);
    assert_eq!(s.border_color, lumen_static::style::Palette::LIGHT.accent, "Tabs' seed does not resolve its mark to the accent");
}
