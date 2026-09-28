//! **A text field's caret, placeholder, submit and words** — M57/B15
//! (`docs/plan-widget-fixes.md`).
//!
//! The caret at the end of the text sat *before* the last glyph (`x_of`
//! answered the last cluster's start), a click past the end landed before
//! the last character, there was no placeholder and no submit, and every
//! probe under a new box re-shaped a run that ignores the box
//! (`docs/widget-audit.md`, TextInput).

use core::cell::Cell;

use lumen_static::cx::{BlockMetrics, TextMeasure};
use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::event::{Event, EventCtx, Key, Mods};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, NodeState, Point, Size};
use lumen_static_widgets::TextInput;

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 60.0 };

fn key(t: &mut TextInput, k: Key, mods: Mods) -> bool {
    let ev = Event::Key { key: k, down: true };
    t.on_event(&ev, &EventCtx::for_event_with(0, ev, mods))
}

/// The caret rect's x and the run's measured end, laid out with the
/// system face: `BlockMetrics` shapes no clusters, which is the fallback
/// path this defect was not on.
///
/// **The plan's cause was half right.** The shell's shaper emits an
/// insertion point *after* the last character, so with it the end caret
/// was already correct, and restoring the old fallback alone left this
/// green. The fallback is reached when that cluster is missing — a sink
/// that filled — and there it put the caret at 32 px, inside the `o`.
/// Measured with the end cluster removed from the shaper: old fallback
/// 32 (red), this one 42 (green).
#[cfg(feature = "truetype")]
#[test]
fn the_end_caret_is_after_the_last_glyph() {
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let Some(font) = FontStack::system() else {
        eprintln!("no system font; skipping");
        return;
    };
    let mut t: TextInput = TextInput::new("Hello");
    let mut style = [<TextInput as Widget>::type_defaults(); 1];
    style[0].color = Color(0, 0, 0, 255);
    let mut st = [NodeState::default(); 1];
    let mut m = FontMetrics::new(font);
    let w = m.measure_weighted("Hello", style[0].font_size, style[0].font_weight, f32::INFINITY).w;
    let last = m.measure_weighted("Hell", style[0].font_size, style[0].font_weight, f32::INFINITY).w;
    let mut cx = LayoutCx::new(&mut m);
    t.layout(&mut cx, VIEW, &style, &mut st, 0);
    t.place(Point { x: 0.0, y: 0.0 }, &style, &mut st, 0);
    st[0].focused = true;
    let mut list = DisplayList::new();
    t.paint(&st, 0, &mut list);
    let pad = style[0].padding.left;
    let caret = list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Rect { rect, .. } if (rect[2] - rect[0] - 1.0).abs() < 1e-3 => Some(rect[0] - pad),
            _ => None,
        })
        .expect("setup: no caret drawn");
    println!("caret at {caret}, 'Hell' ends at {last}, 'Hello' at {w}");
    assert!((caret - w).abs() < 1.0, "the end caret is at {caret}, not after the last glyph ({w})");
}

#[derive(Default)]
struct Counting {
    inner: BlockMetrics,
    calls: Cell<u32>,
}

impl TextMeasure for Counting {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        self.inner.measure(text, size, max_w)
    }
    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        self.inner.measure_weighted(text, size, weight, max_w)
    }
}

#[test]
fn a_probe_under_a_new_box_does_not_reshape() {
    let mut t: TextInput = TextInput::new("some text");
    let style = [<TextInput as Widget>::type_defaults(); 1];
    let mut st = [NodeState::default(); 1];
    let mut m = Counting::default();
    for max_w in [300.0, 1e6, 2e6, 300.0] {
        let mut cx = LayoutCx::new(&mut m);
        t.layout(&mut cx, Constraints { max_w, ..VIEW }, &style, &mut st, 0);
    }
    println!("{} measure calls over four layouts at three widths", m.calls.get());
    assert_eq!(m.calls.get(), 1, "the run was re-shaped for a box it ignores");
}

#[test]
fn a_placeholder_shows_only_while_empty() {
    let mut t: TextInput = TextInput::default();
    t.placeholder = "Search…";
    let mut style = [<TextInput as Widget>::type_defaults(); 1];
    style[0].color = Color(0x1b, 0x22, 0x30, 0xff);
    let mut st = [NodeState::default(); 1];
    let mut m = BlockMetrics;
    let run = |t: &mut TextInput, st: &mut [NodeState; 1], m: &mut BlockMetrics| {
        let mut cx = LayoutCx::new(m);
        t.layout(&mut cx, VIEW, &style, st, 0);
        let mut list = DisplayList::new();
        t.paint(st, 0, &mut list);
        list.cmds
            .into_iter()
            .filter_map(|c| match c {
                Cmd::Text { color, .. } => Some(color),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let empty = run(&mut t, &mut st, &mut m);
    t.set_str("x");
    let full = run(&mut t, &mut st, &mut m);
    println!("empty draws {empty:?}; with text {full:?}");
    assert_eq!(empty.len(), 1, "no placeholder drawn while empty");
    assert!(empty[0].3 < 0xff, "the placeholder is drawn as text, not dimmed");
    assert_eq!(full, vec![style[0].color], "the placeholder outlived the text");
    // Its contrast on the field's ground, in both palettes: the text role
    // at the placeholder's alpha over `raised`.
    fn lum(c: (f32, f32, f32)) -> f32 {
        let f = |v: f32| {
            let v = v / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
    }
    let a = lumen_static_widgets::input::PLACEHOLDER_ALPHA as f32 / 255.0;
    for (name, p) in [("light", lumen_static::style::Palette::LIGHT), ("dark", lumen_static::style::Palette::DARK)] {
        let (t, g) = (p.text, p.raised);
        let mix = |c: u8, b: u8| c as f32 * a + b as f32 * (1.0 - a);
        let (lp, lg) = (lum((mix(t.0, g.0), mix(t.1, g.1), mix(t.2, g.2))), lum((g.0 as f32, g.1 as f32, g.2 as f32)));
        let ratio = (lp.max(lg) + 0.05) / (lp.min(lg) + 0.05);
        println!("placeholder on {name} raised: {ratio:.2}:1 (bound 4.5)");
        assert!(ratio >= 4.5, "the placeholder reads {ratio:.2}:1 in the {name} palette");
    }
}

#[test]
fn enter_submits_once_and_words_move() {
    let mut t: TextInput = TextInput::new("one two three");
    assert!(!t.take_submitted(), "setup: submitted before Enter");
    key(&mut t, Key::Enter, Mods::NONE);
    assert!(t.take_submitted() && !t.take_submitted(), "Enter did not submit exactly once");
    key(&mut t, Key::Left, Mods::CTRL);
    let a = t.caret();
    key(&mut t, Key::Left, Mods::CTRL);
    let b = t.caret();
    key(&mut t, Key::Right, Mods::CTRL);
    let c = t.caret();
    println!("Ctrl+Left {a}, Ctrl+Left {b}, Ctrl+Right {c}");
    assert_eq!((a, b, c), (8, 4, 7), "word movement did not stop at word edges");
    // A plain Left is one character.
    key(&mut t, Key::Left, Mods::NONE);
    assert_eq!(t.caret(), 6);
}

#[test]
fn a_double_click_selects_the_word() {
    let mut t: TextInput = TextInput::new("one two three");
    let style = [<TextInput as Widget>::type_defaults(); 1];
    let mut st = [NodeState::default(); 1];
    let mut m = BlockMetrics;
    let mut cx = LayoutCx::new(&mut m);
    t.layout(&mut cx, VIEW, &style, &mut st, 0);
    // No clusters under BlockMetrics, so the click lands at the end:
    // "three".
    let ev = Event::PointerDown { x: 0.0, y: 0.0, button: lumen_static::PointerButton::Left, clicks: 2 };
    let mut ctx = EventCtx::for_event(0, ev);
    ctx.local = Point { x: 999.0, y: 5.0 };
    t.on_event(&ev, &ctx);
    println!("double-click selected {:?}", t.selected_str());
    assert_eq!(t.selected_str(), Some("three"));
}
