//! What `Style::visible = false` promises, asserted against the code.
//!
//! `style.rs` documents it as: *"Set false by `Option`/`Either` and by
//! explicit hiding; a hidden subtree takes zero space and is skipped by
//! paint and hit testing."* On 2026-09-02 all three clauses were false,
//! and two independent reviewers found it before any test did:
//!
//! - no leaf read the flag (`ops.rs` had zero occurrences of it), so
//!   hiding a `Label` changed nothing at all;
//! - the derive's early-return zeroed only the container's own slot, so
//!   descendants kept the previous frame's geometry and `paint` walked
//!   straight into them — the hidden subtree stayed on screen at its old
//!   position, and the journal and audit read it too;
//! - `Patch::visible` and the cascade's write were the field's only two
//!   write paths and both had zero coverage.
//!
//! This file is the documented claim, clause by clause.
//! `docs/plan-review-fixes.md` F1.6.

use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{pass, Cmd, Constraints, DisplayList, NodeState, StaticTree, Style, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    #[widget_id("secret")]
    secret: Label,
    mark: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    #[widget_id("panel")]
    panel: Panel,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

struct Frame {
    ui: Ui,
    st: [NodeState; Ui::NODE_COUNT],
    list: DisplayList,
}

impl Frame {
    fn hit_at(&self, y: f32) -> Option<lumen_static::Hit> {
        pass::hit_test(&self.ui, &self.st, 2.0, y)
    }
}

/// Lay out and paint, optionally hiding one node by author id.
fn frame(hide: Option<&'static str>) -> Frame {
    let mut ui = Ui::default();
    ui.head.set("HEAD");
    ui.panel.secret.set("SECRET");
    ui.panel.mark = Fixed { w: 24.0, h: 24.0 };

    let mut styles: Vec<Style> = pass::default_styles(&ui);
    if let Some(id) = hide {
        let sheet = [Rule::new(Selector::Id(id), Patch::new().visible(false))];
        lumen_static::style::cascade(
            <Ui as Widget>::SUB_TABLE,
            <Ui as Widget>::SEEDS,
            &sheet,
            &mut styles,
        );
    }
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    Frame { ui, st, list }
}

/// Every string the frame would render.
fn texts(list: &DisplayList) -> Vec<String> {
    list.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { text, .. } => Some(list.str_of(*text).to_owned()),
            _ => None,
        })
        .collect()
}

/// Total inked area — a rect or glyph run with no area draws nothing.
fn drawn_area(list: &DisplayList) -> f32 {
    list.cmds
        .iter()
        .map(|c| match c {
            Cmd::Rect { rect, .. } | Cmd::Text { rect, .. } => {
                (rect[2] - rect[0]).max(0.0) * (rect[3] - rect[1]).max(0.0)
            }
            _ => 0.0,
        })
        .sum()
}

#[test]
fn a_hidden_container_takes_zero_space() {
    let shown = frame(None);
    let hidden = frame(Some("panel"));
    assert!(shown.st[0].size.h > hidden.st[0].size.h, "the column shrank");
    assert_eq!(
        hidden.st[0].size.h,
        shown.st[1].size.h,
        "…to exactly the height of the head label alone"
    );
    assert_eq!(hidden.st[2].size, lumen_static::Size::default(), "the panel is zero-sized");
}

#[test]
fn a_hidden_container_is_skipped_by_paint_including_its_children() {
    let hidden = frame(Some("panel"));
    assert!(texts(&hidden.list).contains(&"HEAD".to_owned()), "the visible label still draws");
    assert!(
        !texts(&hidden.list).contains(&"SECRET".to_owned()),
        "the hidden child's text must not reach the screen: {:?}",
        texts(&hidden.list)
    );
    // The child `Fixed` is a rect, not text — assert on area instead.
    let shown_area = drawn_area(&frame(None).list);
    assert!(drawn_area(&hidden.list) < shown_area, "less is drawn than when shown");
}

#[test]
fn a_hidden_container_is_skipped_by_hit_testing() {
    let shown = frame(None);
    let panel_top = shown.st[2].pos.y + 1.0;
    assert!(shown.hit_at(panel_top).is_some(), "hittable when shown");

    let hidden = frame(Some("panel"));
    assert!(hidden.hit_at(panel_top).is_none(), "and not hittable when hidden");
}

#[test]
fn a_hidden_leaf_takes_zero_space_and_does_not_draw() {
    // The clause that failed hardest: no leaf read the flag, so hiding a
    // label changed literally nothing.
    let shown = frame(None);
    let hidden = frame(Some("head"));
    assert_eq!(hidden.st[1].size, lumen_static::Size::default(), "the label is zero-sized");
    assert!(hidden.st[0].size.h < shown.st[0].size.h, "and the column shrank");
    assert!(
        !texts(&hidden.list).contains(&"HEAD".to_owned()),
        "its text must not draw: {:?}",
        texts(&hidden.list)
    );
    assert!(texts(&hidden.list).contains(&"SECRET".to_owned()), "siblings are unaffected");
}
