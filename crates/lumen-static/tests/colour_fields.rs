//! M25/T25.10: the colours one widget needs are fields on that widget.
//!
//! Each was derived from the text colour because a `Style` field would be
//! paid by every node. Each test sets a colour the derivation could not
//! produce, asserts it differs from what the widget paints by default, and
//! reads it back off the display list — so a paint that ignored the field
//! fails.

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static_widgets::input::selection_tint;
use lumen_static_widgets::ops::Fixed;
use lumen_static::{Constraints, LayoutCx, Length, Node, NodeState, Point, Style};
use lumen_static_widgets::{ScrollView, TextArea, TextInput};

const RED: Color = Color(200, 30, 30, 255);
const TEAL: Color = Color(20, 160, 150, 120);

/// A measurer that reports one cluster per character, wrapping at a fixed
/// column — `tests/textarea.rs`'s stand-in. `TextArea` paints a selection
/// from the clusters layout recorded, and `BlockMetrics` records none, so
/// without this a selection paints nothing and the colour could not show.
struct Wrapping;

impl lumen_static::TextMeasure for Wrapping {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> lumen_static::Size {
        let per = ((max_w / 8.0).floor().max(1.0)) as usize;
        let lines = text.len().div_ceil(per).max(1);
        lumen_static::Size { w: max_w, h: size * lines as f32 }
    }
    fn shape(
        &mut self,
        text: &str,
        _size: f32,
        _weight: f32,
        max_w: f32,
        sink: &mut dyn lumen_static::GlyphSink,
    ) -> lumen_static::Size {
        let per = ((max_w / 8.0).floor().max(1.0)) as usize;
        for (i, (b, _)) in text.char_indices().enumerate() {
            sink.cluster(lumen_static::Cluster {
                byte: b as u32,
                x: (i % per) as f32 * 8.0,
                line: (i / per) as u16,
                rtl: false,
            });
        }
        self.measure(text, 16.0, max_w)
    }
    fn line_height(&mut self, size: f32) -> f32 {
        size
    }
}

/// Lay `w` out in a 200×100 box under `styles`, with node 0's focus as
/// given, and return every `Rect` colour it painted, in order.
fn rects<N: Node>(w: &mut N, styles: &[Style], focused: bool) -> Vec<Color> {
    rects_with(w, styles, focused, &mut lumen_static::cx::BlockMetrics)
}

fn rects_with<N: Node>(
    w: &mut N,
    styles: &[Style],
    focused: bool,
    metrics: &mut dyn lumen_static::TextMeasure,
) -> Vec<Color> {
    let mut st = vec![NodeState::default(); styles.len()];
    st[0].focused = focused;
    let limits = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };
    let mut cx = LayoutCx::new(metrics);
    w.layout(&mut cx, limits, styles, &mut st, 0);
    w.place(Point { x: 0.0, y: 0.0 }, styles, &mut st, 0);
    let mut l = DisplayList::new();
    w.paint(&st, 0, &mut l);
    l.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Rect { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

#[test]
fn a_text_input_paints_its_caret_and_selection_colours() {
    let style = TextInput::<String>::default().defaults();
    let fg = style.color;
    assert_ne!(RED, fg, "setup: the caret colour differs from the text colour it defaults to");
    assert_ne!(
        TEAL,
        selection_tint(fg),
        "setup: the selection colour differs from the default tint"
    );

    let mut caret = TextInput::<String>::new("abc");
    caret.caret_color = Some(RED);
    let painted = rects(&mut caret, &[style], true);
    println!("caret: {painted:?}");
    assert_eq!(painted.last(), Some(&RED), "the caret ignored `caret_color`");

    let mut sel = TextInput::<String>::new("abc");
    assert!(sel.select_all(), "setup: there is text to select");
    sel.selection_color = Some(TEAL);
    let painted = rects(&mut sel, &[style], false);
    println!("selection: {painted:?}");
    assert_eq!(painted.first(), Some(&TEAL), "the selection ignored `selection_color`");
}

#[test]
fn a_text_area_paints_its_caret_and_selection_colours() {
    let style = TextArea::<String>::default().defaults();
    let mut caret = TextArea::<String>::new("ab\ncd");
    caret.caret_color = Some(RED);
    let painted = rects(&mut caret, &[style], true);
    println!("caret: {painted:?}");
    assert_eq!(painted.last(), Some(&RED), "the caret ignored `caret_color`");

    let mut sel = TextArea::<String>::new("ab\ncd");
    assert!(sel.select_all(), "setup: there is text to select");
    sel.selection_color = Some(TEAL);
    let painted = rects_with(&mut sel, &[style], false, &mut Wrapping);
    println!("selection: {painted:?}");
    assert!(!painted.is_empty(), "setup: a selection was painted");
    assert!(
        painted.iter().all(|c| *c == TEAL),
        "the selection ignored `selection_color`: {painted:?}"
    );
}

#[test]
fn a_scroll_view_paints_its_track_and_thumb_colours() {
    let mut view = ScrollView::new(Fixed { w: 50.0, h: 500.0 });
    let styles = [
        Style { width: Length::Fixed(100.0), height: Length::Fixed(100.0), ..view.defaults() },
        view.content.defaults(),
    ];
    let default_parts = rects(&mut view, &styles, false);
    let (d_track, d_thumb) =
        (default_parts[default_parts.len() - 2], default_parts[default_parts.len() - 1]);
    println!("default track {d_track:?} thumb {d_thumb:?}");
    assert_ne!(RED, d_thumb, "setup: the thumb colour differs from the default");
    assert_ne!(TEAL, d_track, "setup: the track colour differs from the default");

    view.track = Some(TEAL);
    view.thumb = Some(RED);
    let parts = rects(&mut view, &styles, false);
    println!("set track {:?} thumb {:?}", parts[parts.len() - 2], parts[parts.len() - 1]);
    assert_eq!(parts[parts.len() - 2], TEAL, "the track ignored `track`");
    assert_eq!(parts[parts.len() - 1], RED, "the thumb ignored `thumb`");
}
