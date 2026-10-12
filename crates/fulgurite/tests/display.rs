//! M5 seams: paint emits drawing commands, and layout measures through a
//! context that a real shaping stack can be dropped into.

use fulgurite::cx::{LayoutCx, TextMeasure};
use fulgurite::display::{Cmd, DisplayList, Renderer};
use fulgurite_widgets::ops::{Button, Fixed, Label};
use fulgurite::{pass, Constraints, NodeState, Size, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    head: Label,
    dot: Fixed,
    go: Button<()>,
}

#[test]
fn paint_emits_commands_a_renderer_can_consume() {
    let mut ui = Ui::default();
    ui.head.set("hello");
    ui.go.label = "go";

    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&ui);
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);

    // A label becomes text, a fixed box becomes a rect, a button becomes
    // both — and the text is addressable through the list's own buffer,
    // so no command owns a `String`.
    let texts: Vec<&str> = list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { text, .. } => Some(list.str_of(*text)),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["hello", "go"]);
    assert!(list.cmds.iter().any(|c| matches!(c, Cmd::Rect { .. })));

    // Commands carry absolute geometry, so a renderer needs nothing else.
    if let Some(Cmd::Text { rect, .. }) = list.cmds.first() {
        assert_eq!(rect[0], st[1].pos.x);
        assert_eq!(rect[3] - rect[1], st[1].size.h);
    } else {
        panic!("expected the head label first, got {:?}", list.cmds.first());
    }
}

#[test]
fn the_list_is_reused_between_frames() {
    let mut ui = Ui::default();
    ui.head.set("frame");
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&ui);
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    let n = list.len();
    let first = list.hash_from(0);

    // clear() keeps the allocations; the same frame hashes the same.
    list.clear();
    assert!(list.is_empty());
    pass::paint(&ui, &st, &mut list);
    assert_eq!(list.len(), n);
    assert_eq!(list.hash_from(0), first, "an unchanged frame paints identically");
}

/// A renderer is injected where the list is *consumed*, never in a
/// widget signature — the property the architecture answer promised.
#[derive(Default)]
struct CountingRenderer {
    rects: usize,
    glyphs: usize,
}

impl Renderer for CountingRenderer {
    fn render(&mut self, list: &DisplayList, _w: u32, _h: u32) {
        for c in &list.cmds {
            match c {
                Cmd::Rect { .. } => self.rects += 1,
                Cmd::Text { text, .. } => self.glyphs += list.str_of(*text).len(),
                _ => {}
            }
        }
    }
}

#[test]
fn a_renderer_plugs_in_at_the_consumption_site() {
    let mut ui = Ui::default();
    ui.head.set("abc");
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&ui);
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);

    let mut r = CountingRenderer::default();
    let dynr: &mut dyn Renderer = &mut r;
    dynr.render(&list, 400, 400);
    assert!(r.rects >= 1 && r.glyphs >= 3);
}

/// A stand-in for a real shaping stack: wide glyphs, so the layout
/// visibly depends on the injected metrics rather than on a constant.
struct WideMetrics;

impl TextMeasure for WideMetrics {
    fn measure(&mut self, text: &str, size: f32, _max_w: f32) -> Size {
        Size { w: text.len() as f32 * size, h: size }
    }
}

#[test]
fn layout_measures_through_the_injected_context() {
    let mut ui = Ui::default();
    ui.head.set("wide");
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&ui);

    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let with_default = st[1].size.w;

    // Same text, different metrics: the width follows the context, which
    // is the seam a parley/swash stack drops into (parity Phase A).
    ui.head.set("wide"); // re-mark dirty so the memo does not answer
    let mut m = WideMetrics;
    let mut cx = LayoutCx::new(&mut m);
    pass::layout_with(&mut ui, &mut cx, &sty, &mut st, VIEW);
    assert_eq!(st[1].size.w, 4.0 * 16.0);
    assert_ne!(st[1].size.w, with_default);
}
