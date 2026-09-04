//! M11/T11.3: the text area.
//!
//! The property that makes it a separate widget rather than a `wrap`
//! flag: **up and down are not byte steps**. They are "the cluster on
//! that line whose x is nearest mine", which only exists because the
//! shaping seam reports `Cluster::line` (T7.2b) — and which the
//! stand-in measurer cannot answer, so those tests say so rather than
//! pretending.

use lumen_static::cx::{BlockMetrics, LayoutCx};
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::textarea::TextArea;
use lumen_static::widget::Node;
use lumen_static::{pass, Cmd, Constraints, DisplayList, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 120.0, min_h: 0.0, max_h: 200.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("body")]
    body: TextArea,
}

fn laid(ui: &mut Ui) -> [NodeState; Ui::NODE_COUNT] {
    let sty = pass::default_styles(ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(ui, &sty, &mut st, VIEW);
    st
}

fn send(t: &mut TextArea, ev: Event) -> bool {
    t.on_event(&ev, &EventCtx::for_event(0, ev))
}

fn typed(t: &mut TextArea, s: &str) {
    for ch in s.chars() {
        send(t, Event::Text { ch });
    }
}

#[test]
fn enter_inserts_a_newline_where_a_single_line_field_refuses_one() {
    // The one key that must behave oppositely in the two widgets, and
    // the clearest argument that they are two widgets.
    let mut t: TextArea = TextArea::default();
    typed(&mut t, "ab");
    assert!(send(&mut t, Event::Key { key: Key::Enter, down: true }));
    typed(&mut t, "cd");
    assert_eq!(t.as_str(), "ab\ncd");

    // A single-line field declines Enter, so a form's submit handler
    // still sees it.
    let mut f: lumen_static::input::TextInput = lumen_static::input::TextInput::new("ab");
    assert!(!f.on_event(
        &Event::Key { key: Key::Enter, down: true },
        &EventCtx::for_event(0, Event::Key { key: Key::Enter, down: true })
    ));
    assert_eq!(f.as_str(), "ab");
}

#[test]
fn edits_cross_whole_characters() {
    let mut t: TextArea = TextArea::new("aé漢");
    assert!(t.backspace());
    assert_eq!(t.as_str(), "aé");
    assert!(t.backspace());
    assert_eq!(t.as_str(), "a", "the two-byte char went whole");
    // Walk the caret and slice at every stop: off a boundary panics.
    let mut t2: TextArea = TextArea::new("aé漢🎉");
    while t2.move_left() {
        assert!(t2.as_str().is_char_boundary(t2.caret()));
        let _ = &t2.as_str()[..t2.caret()];
    }
    assert_eq!(t2.caret(), 0);
}

#[test]
fn a_run_longer_than_the_box_wraps_and_the_widget_grows_to_its_style() {
    let mut ui = Ui::default();
    ui.body.set_str(&"wrap ".repeat(40));
    let st = laid(&mut ui);
    // The default height is fixed, so the box does not grow with the
    // text — that is what makes it scroll.
    assert_eq!(st[1].size.h, 96.0, "the box is its styled height");
    assert!(st[1].size.w <= VIEW.max_w + 0.001);
}

#[test]
fn the_overflowing_run_is_clipped_and_the_caret_only_drawn_when_focused() {
    let mut ui = Ui::default();
    ui.body.set_str(&"wrap ".repeat(40));
    let mut st = laid(&mut ui);

    let count = |ui: &Ui, st: &[NodeState]| {
        let mut l = DisplayList::new();
        pass::paint(ui, st, &mut l);
        (
            l.cmds.iter().filter(|c| matches!(c, Cmd::PushClip { .. })).count(),
            l.cmds.iter().filter(|c| matches!(c, Cmd::Rect { .. })).count(),
        )
    };
    let (clips, unfocused) = count(&ui, &st);
    assert!(clips >= 1, "the run is clipped to the box");

    st[1].focused = true;
    let (_, focused) = count(&ui, &st);
    assert_eq!(focused, unfocused + 1, "the caret is one more rect, and only when focused");
}

#[test]
fn up_and_down_need_real_line_information_and_say_so_without_it() {
    // `BlockMetrics` reports no clusters (T7.2b's documented default),
    // so there are no lines to move between. The widget must decline
    // rather than move the caret somewhere arbitrary.
    let mut ui = Ui::default();
    ui.body.set_str("one two three four five six seven");
    let _ = laid(&mut ui);
    // Nothing was shaped into clusters, so a line move finds nothing.
    assert!(!ui.body.move_line(1), "no clusters, no line movement");
    assert!(!ui.body.move_line(-1));
}

#[test]
fn up_and_down_move_by_geometry_when_the_measurer_reports_lines() {
    // A stand-in that wraps at a fixed column and reports one cluster
    // per character with a line number — the minimum a real shaper
    // gives, and enough to prove the caret model.
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
                });
            }
            self.measure(text, 16.0, max_w)
        }
        fn line_height(&mut self, size: f32) -> f32 {
            size
        }
    }

    let mut ui = Ui::default();
    ui.body.set_str("abcdefghijklmnopqrstuvwxyz");
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let mut m = Wrapping;
    let mut cx = LayoutCx::new(&mut m);
    pass::layout_with(&mut ui, &mut cx, &sty, &mut st, VIEW);

    // Caret at the end; walk up a line and back down.
    let end_line = ui.body.caret_line();
    assert!(end_line >= 1, "the text wrapped: line {end_line}");
    assert!(ui.body.move_line(-1), "up a line");
    assert_eq!(ui.body.caret_line(), end_line - 1);
    assert!(ui.body.move_line(1), "and back down");
    assert_eq!(ui.body.caret_line(), end_line);
    assert!(!ui.body.move_line(-99), "above the first line, nothing to do");

    // Silence the unused warning for the stand-in path.
    let _ = BlockMetrics;
}
