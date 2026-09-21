//! T14.4: selection in `TextInput`.
//!
//! The interesting property is that **Shift+Arrow and drag produce the
//! same selection**, because they are two routes to one anchor — and
//! the second of them exists only because M11/T11.2's pointer capture
//! routes `PointerMove`, which `input.rs` said for a milestone that it
//! did not.

use lumen_static::event::InputState;
use lumen_static::{pass, Cluster, Constraints, Event, GlyphSink, InlineText, Key, LayoutCx, Mods, NodeState, Size, StaticTree, TextMeasure, Widget};
use lumen_static_widgets::{TextInput};

/// One cluster per character, 8 px apart.
///
/// `BlockMetrics` reports **no** clusters — `TextMeasure::shape`'s
/// default measures and says nothing — so a field laid out with it has
/// no geometry to hit-test against, and every click lands at the end of
/// the run. Anything testing a caret's *position* has to supply a
/// measurer, which is what a real host does too.
struct Monospace;

const ADVANCE: f32 = 8.0;

impl TextMeasure for Monospace {
    fn measure(&mut self, text: &str, size: f32, _max_w: f32) -> Size {
        Size { w: text.chars().count() as f32 * ADVANCE, h: size }
    }
    fn shape(
        &mut self,
        text: &str,
        size: f32,
        _weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        for (i, (b, _)) in text.char_indices().enumerate() {
            sink.cluster(Cluster { byte: b as u32, x: i as f32 * ADVANCE, line: 0 });
        }
        // The insertion point after the last character, as a real
        // shaper reports it.
        sink.cluster(Cluster {
            byte: text.len() as u32,
            x: text.chars().count() as f32 * ADVANCE,
            line: 0,
        });
        self.measure(text, size, max_w)
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput,
}

#[derive(lumen_static::App, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 100.0 };
/// The field is the only child, so index 1.
const FIELD: u32 = 1;

struct F {
    app: App,
    styles: Vec<lumen_static::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    input: InputState,
}

impl F {
    fn new(text: &str) -> Self {
        let mut app = App::default();
        app.ui.field.set_str(text);
        let styles = pass::default_styles(&app.ui);
        let mut f = F {
            app,
            styles,
            st: [NodeState::default(); Ui::NODE_COUNT],
            input: InputState { focus: Some(FIELD), ..Default::default() },
        };
        let mut m = Monospace;
        let mut cx = LayoutCx::new(&mut m);
        pass::layout_with(&mut f.app.ui, &mut cx, &f.styles, &mut f.st, VIEW);
        f
    }

    /// Re-lay out with the cluster-reporting measurer, which is what a
    /// host does every frame.
    fn relayout(&mut self) {
        let mut m = Monospace;
        let mut cx = LayoutCx::new(&mut m);
        pass::layout_with(&mut self.app.ui, &mut cx, &self.styles, &mut self.st, VIEW);
    }

    fn send(&mut self, ev: Event) {
        pass::dispatch(&mut self.app, &self.styles, &mut self.st, &mut self.input, &ev);
    }

    fn field(&self) -> &TextInput {
        &self.app.ui.field
    }

    fn sel(&self) -> Option<&str> {
        self.app.ui.field.selected_str()
    }
}

#[test]
fn there_is_no_selection_until_something_makes_one() {
    let f = F::new("hello world");
    assert_eq!(f.sel(), None);
    assert_eq!(f.field().selection(), None);
}

#[test]
fn shift_arrow_extends_and_a_bare_arrow_collapses() {
    let mut f = F::new("hello");
    f.send(Event::Key { key: Key::Home, down: true });

    f.send(Event::Modifiers(Mods::SHIFT));
    f.send(Event::Key { key: Key::Right, down: true });
    f.send(Event::Key { key: Key::Right, down: true });
    assert_eq!(f.sel(), Some("he"), "two shifted rights select two chars");

    // A bare arrow drops the selection rather than extending it.
    f.send(Event::Modifiers(Mods::NONE));
    f.send(Event::Key { key: Key::Right, down: true });
    assert_eq!(f.sel(), None, "moving without shift collapses");
}

#[test]
fn shift_end_and_shift_home_select_to_the_edges() {
    let mut f = F::new("hello");
    f.send(Event::Key { key: Key::Home, down: true });
    f.send(Event::Modifiers(Mods::SHIFT));
    f.send(Event::Key { key: Key::End, down: true });
    assert_eq!(f.sel(), Some("hello"));

    f.send(Event::Key { key: Key::Home, down: true });
    assert_eq!(f.sel(), None, "back to where the anchor is, so nothing is selected");
}

#[test]
fn a_selection_is_ordered_whichever_way_it_was_dragged() {
    let mut f = F::new("hello");
    // Caret at the end, then shift-left twice: the anchor is *after*
    // the caret, and the range must still read low..high.
    f.send(Event::Key { key: Key::End, down: true });
    f.send(Event::Modifiers(Mods::SHIFT));
    f.send(Event::Key { key: Key::Left, down: true });
    f.send(Event::Key { key: Key::Left, down: true });
    assert_eq!(f.sel(), Some("lo"));
    let r = f.field().selection().unwrap();
    assert!(r.start < r.end, "a caller can slice with it");
}

#[test]
fn ctrl_a_selects_everything() {
    let mut f = F::new("hello world");
    f.send(Event::Modifiers(Mods::CTRL));
    f.send(Event::Key { key: Key::Char('a'), down: true });
    assert_eq!(f.sel(), Some("hello world"));

    // And Cmd+A, because which modifier is "the" shortcut one is the
    // platform's business (T14.1).
    let mut g = F::new("hello world");
    g.send(Event::Modifiers(Mods::META));
    g.send(Event::Key { key: Key::Char('a'), down: true });
    assert_eq!(g.sel(), Some("hello world"));
}

#[test]
fn escape_clears_the_selection_but_keeps_the_caret() {
    let mut f = F::new("hello");
    f.send(Event::Modifiers(Mods::CTRL));
    f.send(Event::Key { key: Key::Char('a'), down: true });
    let caret = f.field().caret();
    f.send(Event::Modifiers(Mods::NONE));
    f.send(Event::Key { key: Key::Escape, down: true });
    assert_eq!(f.sel(), None);
    assert_eq!(f.field().caret(), caret, "the caret does not jump");
}

#[test]
fn typing_replaces_the_selection() {
    let mut f = F::new("hello");
    f.send(Event::Modifiers(Mods::CTRL));
    f.send(Event::Key { key: Key::Char('a'), down: true });
    f.send(Event::Modifiers(Mods::NONE));
    f.send(Event::Text { ch: 'X' });
    assert_eq!(f.field().as_str(), "X", "the selection went, the character landed");
    assert_eq!(f.sel(), None);
}

#[test]
fn backspace_and_delete_take_the_selection_whole() {
    for key in [Key::Backspace, Key::Delete] {
        let mut f = F::new("hello world");
        f.send(Event::Key { key: Key::Home, down: true });
        f.send(Event::Modifiers(Mods::SHIFT));
        for _ in 0..5 {
            f.send(Event::Key { key: Key::Right, down: true });
        }
        assert_eq!(f.sel(), Some("hello"));
        f.send(Event::Modifiers(Mods::NONE));
        f.send(Event::Key { key, down: true });
        assert_eq!(f.field().as_str(), " world", "{key:?} removed the whole selection");
    }
}

/// **The property the whole task turns on.** T11.2's capture routes
/// `PointerMove` to the pressed node, so a drag selects — the mechanism
/// `input.rs` claimed did not exist.
#[test]
fn a_drag_selects_the_same_span_shift_arrow_does() {
    let mut by_key = F::new("hello");
    by_key.send(Event::Key { key: Key::Home, down: true });
    by_key.send(Event::Modifiers(Mods::SHIFT));
    for _ in 0..3 {
        by_key.send(Event::Key { key: Key::Right, down: true });
    }
    let want = by_key.sel().map(str::to_owned);
    assert_eq!(want.as_deref(), Some("hel"));

    // The same span by dragging. Coordinates come from the laid-out
    // field, and the caret lands on the nearest cluster boundary.
    let mut by_drag = F::new("hello");
    by_drag.relayout();
    let s = by_drag.st[FIELD as usize];
    let y = s.pos.y + s.size.h / 2.0;
    // The run starts one padding in; press on the first cluster and
    // release on the fourth, which is three characters across.
    let pad = by_drag.styles[FIELD as usize].padding.left;
    let x0 = s.pos.x + pad;
    by_drag.send(Event::down(x0, y));
    by_drag.send(Event::PointerMove { x: x0 + 3.0 * 8.0, y });
    by_drag.send(Event::up(x0 + 3.0 * 8.0, y));
    assert_eq!(by_drag.sel(), want.as_deref(), "drag and Shift+Arrow agree");
}

#[test]
fn a_press_without_a_drag_is_a_caret_not_an_empty_selection() {
    let mut f = F::new("hello");
    let s = f.st[FIELD as usize];
    let (x, y) = (s.pos.x + 10.0, s.pos.y + s.size.h / 2.0);
    f.send(Event::down(x, y));
    f.send(Event::up(x, y));
    assert_eq!(f.sel(), None, "a click leaves no selection behind");
}

#[test]
fn selection_survives_a_fixed_capacity_storage() {
    // The same machinery on the alloc-free storage, since that is the
    // one that can refuse an insert (T13.5).
    let mut fld: TextInput<InlineText<16>> = TextInput::new("abcdef");
    assert!(fld.select_all());
    assert_eq!(fld.selected_str(), Some("abcdef"));
    assert!(fld.insert("Z"));
    assert_eq!(fld.as_str(), "Z", "typing over a selection replaces it here too");
}

// ---- TextArea shares the model ---------------------------------------

/// The same anchor model in the multi-line widget, so the two cannot
/// drift. What differs is only what a caret move *means*: `TextArea`
/// has Up and Down, and they extend a selection like any other move.
#[test]
fn textarea_selects_across_lines() {
    use lumen_static_widgets::TextArea;
    let mut a: TextArea = TextArea::default();
    a.set_str("one\ntwo\nthree");

    assert!(a.select_all());
    assert_eq!(a.selected_str(), Some("one\ntwo\nthree"));

    // Typing over it replaces the lot.
    assert!(a.insert("x"));
    assert_eq!(a.as_str(), "x");
    assert_eq!(a.selection(), None);
}

#[test]
fn textarea_backspace_takes_the_selection() {
    use lumen_static_widgets::TextArea;
    let mut a: TextArea = TextArea::default();
    a.set_str("hello");
    a.select_all();
    assert!(a.backspace());
    assert_eq!(a.as_str(), "");
    assert!(!a.backspace(), "nothing left to delete");
}
