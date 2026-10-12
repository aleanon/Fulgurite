//! T14.7: the scrollbar `ScrollView` has shipped without since M8.

use fulgurite_widgets::ops::Fixed;
use fulgurite::{pass, Constraints, DisplayList, NodeState, StaticTree, Widget};
use fulgurite_widgets::{ScrollView};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Tall {
    a: Fixed,
    b: Fixed,
    c: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("scroller")]
    sc: ScrollView<Tall>,
}

/// How many rects the *content* paints: three `Fixed` rows.
const CONTENT_RECTS: usize = 3;

/// Short enough that the content overflows it.
const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };

fn frame(rows: f32) -> (DisplayList, [NodeState; Ui::NODE_COUNT]) {
    let mut ui = Ui::default();
    for f in [&mut ui.sc.content.a, &mut ui.sc.content.b, &mut ui.sc.content.c] {
        f.w = 50.0;
        f.h = rows;
    }
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    (list, st)
}

/// Every `Cmd::Rect` the frame drew.
///
/// The content is three `Fixed`, which paint rects of their own — so
/// the bar is not "the rects", it is the **last two**, painted after
/// `pop_clip`. That ordering is the thing being asserted as much as the
/// geometry: a bar painted inside the clip would be clipped by it.
fn rects(list: &DisplayList) -> Vec<[f32; 4]> {
    list.cmds
        .iter()
        .filter_map(|c| match c {
            fulgurite::Cmd::Rect { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect()
}

#[test]
fn no_overflow_paints_no_scrollbar() {
    // Three 10 px rows in a 100 px viewport: nothing to scroll, and a
    // bar that is always there says "this scrolls" about something that
    // does not.
    let (list, _) = frame(10.0);
    assert_eq!(rects(&list).len(), CONTENT_RECTS, "the three rows, and no bar");
}

#[test]
fn overflow_paints_a_track_and_a_thumb() {
    let (list, st) = frame(200.0);
    let r = rects(&list);
    assert_eq!(r.len(), CONTENT_RECTS + 2, "the rows, plus a track and a thumb");
    let (track, thumb) = (r[CONTENT_RECTS], r[CONTENT_RECTS + 1]);

    let view = st[1];
    let right = view.pos.x + view.size.w;
    assert_eq!(track[2], right, "the track is at the right edge");
    assert_eq!(track[2] - track[0], ScrollView::<Tall>::BAR);
    assert_eq!(track[3] - track[1], view.size.h, "the track spans the viewport");

    let thumb_h = thumb[3] - thumb[1];
    assert!(thumb_h >= ScrollView::<Tall>::MIN_THUMB, "grabbable: {thumb_h}");
    assert!(thumb_h < view.size.h, "and shorter than the track, or it says nothing");
}

#[test]
fn the_thumb_moves_with_the_scroll_and_stays_inside() {
    let mut ui = Ui::default();
    for f in [&mut ui.sc.content.a, &mut ui.sc.content.b, &mut ui.sc.content.c] {
        f.w = 50.0;
        f.h = 200.0;
    }
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];

    let thumb_at = |ui: &mut Ui, st: &mut [NodeState; Ui::NODE_COUNT]| {
        pass::layout(ui, &sty, st, VIEW);
        let mut list = DisplayList::new();
        pass::paint(ui, st, &mut list);
        let r = rects(&list);
        (r[CONTENT_RECTS + 1], r[CONTENT_RECTS])
    };

    let (top, track) = thumb_at(&mut ui, &mut st);
    ui.sc.scroll = f32::MAX; // clamped to the overflow by layout
    let (bottom, _) = thumb_at(&mut ui, &mut st);

    assert!(bottom[1] > top[1], "the thumb moved down");
    assert!(top[1] >= track[1] - 0.01, "and never above the track");
    assert!(bottom[3] <= track[3] + 0.01, "nor below it");
}
