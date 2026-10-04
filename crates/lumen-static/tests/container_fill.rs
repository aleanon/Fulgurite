//! M26/T26.9: a transparent container emits no body command.
//!
//! The derive guarded its shadow and emitted the fill unconditionally,
//! including when the background is fully transparent — so every layout
//! container in a tree contributed one command per frame that retention
//! hashed, kept a slot for, and handed to a sink that dropped it. `hit`
//! has had the same predicate since `examples/glass` found an empty
//! wrapper swallowing the frame's input; this is `paint` agreeing with
//! it.
//!
//! **Both directions.** A container that *is* given a background must
//! still paint one, or the guard is a container that stopped working.

use lumen_static::display::{Cmd, Color, DisplayList, Painter, Rect};
use lumen_static_widgets::ops::Fixed;
use lumen_static::style::{Length, Patch, Rule, Selector, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point, StaticTree};

/// A painter that **counts the calls**, not the commands.
///
/// `DisplayList::rounded_rect` already drops a fully transparent fill —
/// it has since long before this task — so a test that counted
/// *commands* could not see this change at all, and did not: the
/// injection that unguarded the emission left every one of them green.
///
/// That is also the correction to the task's premise. T26.9 said
/// retention "hashes and retains a command the sink dropped"; the sink
/// drops it before retention ever sees it. What the guard actually
/// removes is the **call** — a `dyn` dispatch and two compares per
/// container per frame — which is smaller than the task expected and is
/// only visible here.
#[derive(Default)]
struct Counting {
    list: DisplayList,
    rounded_calls: usize,
}

impl Painter for Counting {
    fn rect(&mut self, rect: Rect, color: Color) {
        self.list.rect(rect, color);
    }
    fn text_spaced(&mut self, rect: Rect, s: &str, size: f32, weight: f32, line_height: u8, color: Color) {
        self.list.text_spaced(rect, s, size, weight, line_height, color);
    }
    fn rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        self.rounded_calls += 1;
        self.list.rounded_rect(rect, color, radius);
    }
    fn shadow(&mut self, rect: Rect, radius: f32, spread: f32) {
        self.list.shadow(rect, radius, spread);
    }
    fn stroked_rect(&mut self, rect: Rect, color: Color, radius: f32, width: f32) {
        self.list.stroked_rect(rect, color, radius, width);
    }
    fn push_clip(&mut self, rect: Rect) {
        self.list.push_clip(rect);
    }
    fn pop_clip(&mut self) {
        self.list.pop_clip();
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    #[widget_id("leaf")]
    leaf: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("wrapper")]
    wrapper: Inner,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

fn painted(sheet: &[Rule]) -> Counting {
    let mut ui = Ui::default();
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    lumen_static::style::cascade_deep(&mut ui, sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut ui, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(&mut ui, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let mut out = Counting::default();
    Node::paint(&ui, &st, 0, &mut out);
    out
}

/// **Calls, not commands** — see `Counting`.
fn rounded(out: &Counting) -> usize {
    out.rounded_calls
}

/// The default: two containers, neither given a background, and the
/// leaf sized so it paints something — so "no commands at all" cannot
/// be what this is measuring.
#[test]
fn a_transparent_container_emits_no_body() {
    let sheet = [Rule::new(
        Selector::Id("leaf"),
        Patch::new()
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(10.0))
            .background(Color(1, 2, 3, 255)),
    )];
    let list = painted(&sheet);
    assert!(!list.list.cmds.is_empty(), "setup: nothing was painted at all");
    assert_eq!(
        rounded(&list),
        0,
        "two transparent containers emitted {} rounded rects; a `Fixed` \
         paints a plain `Rect`, so every one of these is a container's \
         invisible body",
        rounded(&list)
    );
}

/// And a container that is given one still paints it.
#[test]
fn a_container_with_a_background_still_paints_it() {
    let sheet = [
        Rule::new(Selector::Id("wrapper"), Patch::new().background(Color(9, 9, 9, 255))),
        Rule::new(
            Selector::Id("leaf"),
            Patch::new().width(Length::Fixed(40.0)).height(Length::Fixed(10.0)),
        ),
    ];
    let list = painted(&sheet);
    assert_eq!(
        rounded(&list),
        1,
        "the one container with a background painted {} bodies",
        rounded(&list)
    );
    assert!(
        list.list.cmds.iter().any(|c| matches!(c, Cmd::RoundedRect { color, .. } if *color == Color(9, 9, 9, 255))),
        "the body it painted is not the colour it was given"
    );
}

/// A *partly* transparent background is still a background — the guard
/// is on zero alpha, not on "looks faint".
#[test]
fn a_translucent_background_is_still_painted() {
    let sheet = [Rule::new(Selector::Id("wrapper"), Patch::new().background(Color(9, 9, 9, 1)))];
    assert_eq!(rounded(&painted(&sheet)), 1, "alpha 1 is not alpha 0");
}

/// **And retention no longer keeps a slot for it.**
///
/// This is what the task is actually about, and it is *not* visible in
/// a display list: `DisplayList::rounded_rect` drops a transparent fill
/// on the floor, so the command never exists. But `Recording` — the
/// wrapper every retained frame paints through — **hashes and observes
/// before it forwards**, so the dropped command still consumed a slot
/// and contributed an ink rect to the frame's damage. On the embedded
/// profile that is a `Streaming<N>` slot, and overflowing `N` degrades
/// a whole frame to `Damage::All`.
#[test]
fn a_transparent_container_keeps_no_retention_slot() {
    use lumen_static::retain::Recording;

    let sheet = [Rule::new(
        Selector::Id("leaf"),
        Patch::new()
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(10.0))
            .background(Color(1, 2, 3, 255)),
    )];
    let mut ui = Ui::default();
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    lumen_static::style::cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut ui, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(&mut ui, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);

    let mut counted = Counting::default();
    let mut ret = Counted::default();
    {
        let mut rec = Recording::new(&mut counted, &mut ret);
        Node::paint(&ui, &st, 0, &mut rec);
    }
    assert!(ret.n > 0, "setup: retention observed nothing at all");
    assert_eq!(
        ret.n, 1,
        "retention kept {} slots for a tree whose only visible command is \
         the leaf's — two transparent containers are still being observed",
        ret.n
    );
}

/// A `Retention` that only counts.
#[derive(Default)]
struct Counted {
    n: usize,
}

impl lumen_static::retain::Retention for Counted {
    fn observe(&mut self, _hash: u64, _ink: lumen_static::display::Rect) {
        self.n += 1;
    }
    fn finish(&mut self) -> lumen_static::retain::Damage {
        lumen_static::retain::Damage::All
    }
    fn take_damage_for_age(&mut self, _age: u8) -> lumen_static::retain::Damage {
        lumen_static::retain::Damage::All
    }
    fn reset(&mut self) {
        self.n = 0;
    }
    fn still(&mut self) -> lumen_static::retain::Damage {
        lumen_static::retain::Damage::All
    }
}
