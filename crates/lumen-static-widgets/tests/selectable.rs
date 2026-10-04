//! **Read-only selection** — M80/T80.4 (`docs/plan-text-fit.md`,
//! "`Selectable<S>` and `app.copy`").
//!
//! A `Selectable` is a `Label` that takes focus and answers
//! `Node::selection` at the index it shares with its label, so the host's
//! copy — which asks the focused index — gets what the pointer selected.
//! Every gate here goes through a `Host` laid out with the pinned face
//! (`FontMetrics`), so a press lands where a window's would and the rows
//! are the rows the software canvas paints.
//!
//! - **copy**: a press and a drag over two lines copy exactly the dragged
//!   substring; a double press one word, a triple the whole text; a press
//!   on a plain `Label` beside it is not handled and copies nothing.
//! - **rows**: the tint's row on each line sits where that line's ink is,
//!   at `line_height` 0 and 24 — measured as line k's (ink top − tint top)
//!   against line 1's, so the glyphs' own offset inside a row cancels.
//! - **sizes**: `size_of::<Label>()` is what T80.3 left it.
//! - **the bind seam**: a bind that shortens the text clamps the selection.
//!
//! Ctrl+X degrading to copy is gated in `lumen-static-winit`'s
//! `tests/clipboard_shortcut.rs`, which can name the window's shortcut
//! decision.
//!
//! A still frame reshaping nothing is `tests/selectable_idle.rs`, alone in
//! its binary because the text counters are process-wide.

use lumen_static::display::{Cmd, Color};
use lumen_static::event::{Event, Key, Mods, PointerButton};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, StaticTree, TextMeasure};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;
use lumen_static_widgets::Selectable;

#[path = "../../lumen-static-shell/tests/fonts/pinned.rs"]
mod pinned;

const PX: f32 = 16.0;
/// Two lines by a hard break, so the drag below crosses one.
const TWO: &str = "Hold the quick brown fox\nLid jumps over the lazy dog";

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("sel")]
    sel: Selectable<String>,
    #[widget_id("plain")]
    plain: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Page {
    #[tree]
    ui: Ui,
}

fn stack() -> FontStack {
    FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses")
}

fn index(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as lumen_static::Widget>::SUB_TABLE, wid).expect(wid)
}

fn host(font: &FontStack, text: &str, lh: u8) -> (Host<Page>, FontMetrics) {
    let mut a = Page::default();
    a.ui.sel.label.set(text.to_owned());
    a.ui.plain.set("a plain label".to_owned());
    let mut h: Host<Page> = Host::new(a, 480, 240);
    let mut p = Patch::new().color(Color(0, 0, 0, 255)).font_size(PX);
    if lh != 0 {
        p = p.line_height(lh as f32 / 16.0);
    }
    h.set_stylesheet(&[Rule::new(Selector::Id("sel"), p), Rule::new(Selector::Id("plain"), Patch::new().font_size(PX))]);
    let mut m = FontMetrics::new(font.clone());
    h.pump_with(&mut m);
    (h, m)
}

/// The window point of byte offset `at` on `line`, at the middle of that
/// line's row: the content box's origin, the prefix's width, the row.
fn point(h: &Host<Page>, m: &mut FontMetrics, line: &str, at: usize, row: usize) -> (f32, f32) {
    let st = h.states[index("sel")];
    let x = m.measure_weighted(&line[..at], PX, 400.0, f32::INFINITY).w;
    let pitch = m.line_height(PX).ceil();
    (st.pos.x + st.pad.0 + x, st.pos.y + st.pad.1 + pitch * (row as f32 + 0.5))
}

fn press(h: &mut Host<Page>, m: &mut FontMetrics, (x, y): (f32, f32), clicks: u8) -> bool {
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks });
    let a = h.pump_with(m);
    h.post(Event::up(x, y));
    h.pump_with(m);
    a
}

/// **The copy is exactly what was dragged** — and a double press is a
/// word, a triple the whole text, a press on a plain label nothing.
#[test]
fn a_drag_copies_exactly_the_dragged_substring() {
    let font = stack();
    let (mut h, mut m) = host(&font, TWO, 0);
    let (l1, l2) = TWO.split_once('\n').unwrap();
    let from = point(&h, &mut m, l1, "Hold the ".len(), 0);
    let to = point(&h, &mut m, l2, "Lid jumps".len(), 1);
    h.post(Event::PointerDown { x: from.0, y: from.1, button: PointerButton::Left, clicks: 1 });
    let down = h.pump_with(&mut m);
    h.post(Event::PointerMove { x: to.0, y: to.1 });
    let moved = h.pump_with(&mut m);
    h.post(Event::up(to.0, to.1));
    h.pump_with(&mut m);
    let focus = h.input.focus;
    let want = "quick brown fox\nLid jumps";
    let got = h.copy().map(str::to_owned);
    println!(
        "setup: press handled {down}, drag handled {moved}; focus {focus:?} (the Selectable is {}); \
         copy {got:?} ({} bytes) against {want:?} ({} bytes)",
        index("sel"),
        got.as_deref().map_or(0, str::len),
        want.len()
    );
    assert!(down && moved, "setup: the press or the drag was not handled");
    assert_eq!(focus, Some(index("sel") as u32), "setup: the press did not focus the Selectable");
    assert_eq!(got.as_deref(), Some(want), "the copy is not the dragged substring");

    // A double press: the word under it.
    let at = point(&h, &mut m, l2, "Lid jumps ov".len(), 1);
    press(&mut h, &mut m, at, 1);
    let two = press(&mut h, &mut m, at, 2);
    let word = h.copy().map(str::to_owned);
    println!("double press handled {two}: copy {word:?} (want \"over\")");
    assert_eq!(word.as_deref(), Some("over"));

    // A triple press: everything.
    let three = press(&mut h, &mut m, at, 3);
    let all = h.copy().map(str::len);
    println!("triple press handled {three}: copy {all:?} bytes (want {})", TWO.len());
    assert_eq!(h.copy(), Some(TWO));

    // Accept direction: a plain label beside it takes no press, and with
    // focus gone (its loss clears the selection) there is nothing to copy.
    let p = h.states[index("plain")];
    let plain = press(&mut h, &mut m, (p.pos.x + 4.0, p.pos.y + p.size.h / 2.0), 1);
    let after = h.copy();
    println!("a press on the plain label: handled {plain}, copy {after:?} (want false, None)");
    assert!(!plain, "a plain label handled a press");
    assert_eq!(after, None, "a press elsewhere left something to copy");
}

/// The first row of each run of **dark** rows (a glyph, not the tint,
/// which is the text colour at 40 alpha) inside the label's box.
fn ink_tops(c: &Canvas, x0: u32, x1: u32, y0: u32, y1: u32) -> Vec<u32> {
    let dark = |x: u32, y: u32| {
        let i = ((y * c.width + x) * 4) as usize;
        c.pixels[i..i + 3].iter().all(|&v| v < 128)
    };
    let mut tops = Vec::new();
    let mut prev = false;
    for y in y0..y1.min(c.height) {
        let r = (x0..x1.min(c.width)).any(|x| dark(x, y));
        if r && !prev {
            tops.push(y);
        }
        prev = r;
    }
    tops
}

/// **The tint sits on the painted rows** — at the face's spacing (0),
/// where the row is the content height over the line count, and at a line
/// height (24), where it is the placer's pitch.
#[test]
fn the_tint_rows_land_on_the_painted_lines() {
    let font = stack();
    // Identical lines with ascenders, so every line's first ink row is the
    // same height above its glyphs' baseline. **Eight**, not two: at 0 the
    // wrong row (`TextMeasure::line_height`, 18.6 px for the pinned face
    // at 16 px, against the painter's ceil'd 19) drifts 0.4 px a row, so
    // line 2 alone cannot see it and line 8 is 2.8 px off.
    const N: usize = 8;
    let text = ["Hold Lid"; N].join("\n");
    let text = text.as_str();
    for lh in [0u8, 24] {
        let (mut h, mut m) = host(&font, text, lh);
        let i = index("sel");
        h.post(Event::Key { key: Key::Tab, down: true });
        h.pump_with(&mut m);
        assert_eq!(h.input.focus, Some(i as u32), "setup: Tab did not reach the Selectable");
        h.post(Event::Modifiers(Mods::CTRL));
        h.post(Event::Key { key: Key::Char('a'), down: true });
        let took = h.pump_with(&mut m);
        h.post(Event::Modifiers(Mods::NONE));
        h.pump_with(&mut m);
        assert!(took && h.copy() == Some(text), "setup @ {lh}: Ctrl+A did not select the text");
        let tint = lumen_static_widgets::selectable::tint_of(Color(0, 0, 0, 255));
        let mut rows: Vec<[f32; 4]> = h
            .list
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::Rect { rect, color } if *color == tint => Some(*rect),
                _ => None,
            })
            .collect();
        rows.sort_by(|a, b| a[1].total_cmp(&b[1]));
        let st = h.states[i];
        let mut canvas = Canvas::with_font(480, 240, font.clone());
        canvas.clear(Color(255, 255, 255, 255));
        h.render(&mut canvas);
        let (x0, x1) = (st.pos.x as u32, (st.pos.x + st.size.w).ceil() as u32);
        let (y0, y1) = (st.pos.y as u32, (st.pos.y + st.size.h).ceil() as u32);
        let ink = ink_tops(&canvas, x0, x1, y0, y1);
        println!("line_height {lh}: tint rows {:?}; ink line tops {ink:?}", rows.iter().map(|r| (r[1], r[3])).collect::<Vec<_>>());
        assert!(rows.len() == N && ink.len() == N, "setup @ {lh}: {} tint rows, {} ink lines (want {N} each)", rows.len(), ink.len());
        let off = |k: usize| ink[k] as f32 - rows[k][1];
        for k in 1..N {
            let drift = (off(k) - off(0)).abs();
            println!(
                "  line {}: ink top - tint top = {:.2} px against line 1's {:.2}: drift {drift:.2} (bound 1)",
                k + 1,
                off(k),
                off(0)
            );
            assert!(drift <= 1.0, "@ {lh}: line {}'s tint is {drift:.2} px off its ink", k + 1);
        }
    }
}

/// The paint split is `pub(crate)` and adds no field: a `Label` is the
/// size T80.3 left it (80 B for `Label<&'static str>`, 88 B for
/// `Label<String>` on a 64-bit host; `docs/results-text.md` M80/T80.3).
#[test]
fn a_label_is_the_size_t80_3_left_it() {
    let (a, b) = (core::mem::size_of::<Label<&'static str>>(), core::mem::size_of::<Label<String>>());
    println!("size_of Label<&str> {a} (T80.3: 80), Label<String> {b} (T80.3: 88)");
    assert_eq!((a, b), (80, 88));
}

/// **A bind that replaces the text clamps the selection** — the
/// `TextTarget` seam. A whole-text selection over a text a bind then
/// shortens must still copy, and copy only what is there.
#[test]
fn a_bind_that_shortens_the_text_clamps_the_selection() {
    use lumen_static::bind::TextTarget;
    let font = stack();
    let (mut h, mut m) = host(&font, TWO, 0);
    let at = point(&h, &mut m, "Hold", 1, 0);
    press(&mut h, &mut m, at, 3);
    let before = h.copy().map(str::len);
    assert_eq!(before, Some(TWO.len()), "setup: the triple press did not select everything");
    let changed = h.app.ui.sel.set_str("Hold");
    let after = h.copy().map(str::to_owned);
    println!("selected {before:?} bytes; the bind wrote \"Hold\" (changed {changed}); copy now {after:?} (want \"Hold\")");
    assert!(changed, "setup: the bind did not change the text");
    assert_eq!(after.as_deref(), Some("Hold"), "the selection was not clamped to the new text");
}
