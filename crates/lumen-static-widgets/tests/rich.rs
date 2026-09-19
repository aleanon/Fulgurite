//! M27/T27.12 — `RichText`, `Markdown`, `RichTextEditor`, `PaintBox`.

use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::{Markdown, RichText};

fn lay<W: Widget + Node>(w: &mut W, max_w: f32) -> Vec<NodeState> {
    let styles = lumen_static::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let limits = Constraints { min_w: 0.0, max_w, min_h: 0.0, max_h: 600.0 };
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, limits, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

/// The rendered text: what a span *covers*, joined.
fn rendered(r: &RichText) -> String {
    (0..r.spans().len())
        .map(|k| r.span_str(k))
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
fn a_word_is_a_span() {
    let r = RichText::new("one two  three");
    assert_eq!(r.spans().len(), 3, "three words, three spans");
    assert_eq!(rendered(&r), "one|two|three");
}

/// **The offsets survive a double space.** `split_whitespace` drops its
/// separators, so an offset accumulated word-by-word runs one byte
/// short for every extra space — and every span after it covers the
/// wrong text.
#[test]
fn the_offsets_are_found_not_accumulated() {
    let r = RichText::new("a  b   c");
    assert_eq!(rendered(&r), "a|b|c");
    let last = r.spans()[2];
    assert_eq!(
        &r.text()[last.range.0 as usize..last.range.1 as usize],
        "c",
        "the last span's range does not cover the last word"
    );
}

/// Text wraps between spans, and the number of lines is the assertion —
/// "it got taller" is true of a widget that simply mis-measured.
#[test]
fn text_wraps_at_the_width_it_is_given() {
    let mut r = RichText::new("alpha bravo charlie delta echo foxtrot golf hotel");
    lay(&mut r, 1000.0);
    assert_eq!(r.lines(), 1, "eight short words in 1000 px should be one line");
    let wide = r.spans().len();
    lay(&mut r, 120.0);
    assert!(r.lines() > 1, "the same words in 120 px did not wrap at all");
    assert_eq!(r.spans().len(), wide, "wrapping changed how many spans there are");
}

#[test]
fn a_taller_wrap_is_a_taller_box() {
    let mut r = RichText::new("alpha bravo charlie delta echo foxtrot golf hotel");
    let one = lay(&mut r, 1000.0)[0].size.h;
    let many = lay(&mut r, 120.0)[0].size.h;
    assert!(many > one, "wrapped to {} lines and stayed {one} px tall", r.lines());
}

// ---- Markdown --------------------------------------------------------

fn spans_of(src: &str) -> Vec<(String, f32, bool, bool, bool)> {
    let m = Markdown::new(src);
    let r = m.rich();
    (0..r.spans().len())
        .map(|k| {
            let s = r.spans()[k];
            (r.span_str(k).to_string(), s.weight, s.code, s.br, s.bullet)
        })
        .collect()
}

#[test]
fn the_markers_are_stripped_from_what_is_drawn() {
    let out = spans_of("a **bold** word and `code` too");
    let text: Vec<&str> = out.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(
        text,
        vec!["a", "bold", "word", "and", "code", "too"],
        "the asterisks and backticks are markup, not text"
    );
    assert_eq!(out[1].1, 700.0, "`**bold**` is bold");
    assert_eq!(out[0].1, 400.0, "and its neighbours are not");
    assert!(out[4].2, "`` `code` `` is code");
    assert!(!out[3].2, "and its neighbours are not");
}

/// **Bold survives more than two words.** A per-word strip opens on the
/// first and closes on the last and leaves everything between plain —
/// which is what the first version did, and which two-word fixtures
/// cannot see.
#[test]
fn bold_carries_across_a_whole_run() {
    let out = spans_of("say **one two three** please");
    let weights: Vec<f32> = out.iter().map(|s| s.1).collect();
    assert_eq!(
        weights,
        vec![400.0, 700.0, 700.0, 700.0, 400.0],
        "spans were {:?}",
        out.iter().map(|s| (&s.0, s.1)).collect::<Vec<_>>()
    );
}

#[test]
fn a_heading_is_bigger_and_bolder_and_its_hashes_are_gone() {
    let m = Markdown::new("# Title\nbody");
    let r = m.rich();
    assert_eq!(r.span_str(0), "Title", "the `#` is markup");
    assert!(r.spans()[0].scale > 1.0, "a heading is scaled up");
    assert_eq!(r.spans()[0].weight, 700.0);
    assert_eq!(r.span_str(1), "body");
    assert_eq!(r.spans()[1].scale, 1.0, "the body is not a heading");
    assert!(r.spans()[1].br, "the body starts its own line");
}

#[test]
fn a_bullet_is_marked_once_per_line() {
    let out = spans_of("- one two\n- three");
    assert!(out[0].4, "the first word of a bullet carries it");
    assert!(!out[1].4, "the second word does not");
    assert!(out[2].4, "and the next bullet does");
    let text: Vec<&str> = out.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(text, vec!["one", "two", "three"], "the `- ` is markup");
}

/// A blank line is a blank line. It is a *span* — empty, with `br` —
/// because that is what a paragraph gap is once text is a run of words.
#[test]
fn a_blank_line_becomes_a_gap_and_not_a_trailing_one() {
    let m = Markdown::new("one\n\ntwo\n\n");
    let r = m.rich();
    let empties = (0..r.spans().len()).filter(|k| r.span_str(*k).is_empty()).count();
    assert_eq!(empties, 1, "one gap between the paragraphs and none after the last");
    assert_eq!(r.span_str(0), "one");
    assert_eq!(r.span_str(2), "two");
}

#[test]
fn the_first_word_opens_no_line() {
    let m = Markdown::new("first\nsecond");
    assert!(!m.rich().spans()[0].br, "a leading break renders a blank first line");
    assert!(m.rich().spans()[1].br, "and the second line does open one");
}

// ---- the editor ------------------------------------------------------

use lumen_static_widgets::{PaintBox, RichTextEditor};

#[test]
fn the_preview_follows_the_text() {
    let mut e = RichTextEditor::new("# Hi\nthere");
    assert_eq!(e.preview.rich().span_str(0), "Hi", "the preview did not parse at all");
    e.input.set_str("**bold** now");
    assert!(e.sync(), "the text changed and sync reported nothing");
    assert_eq!(e.preview.rich().span_str(0), "bold");
    assert_eq!(e.preview.rich().spans()[0].weight, 700.0);
    // And a sync with nothing to do says so — the preview is re-parsed
    // on every layout, and a parser that rebuilt unconditionally would
    // allocate per frame.
    assert!(!e.sync(), "a second sync re-parsed unchanged text");
}

#[test]
fn bold_wraps_the_selection_and_the_preview_shows_it() {
    let mut e = RichTextEditor::new("make me bold");
    e.input.select_all();
    assert!(e.bold(), "bold reported that it wrote nothing");
    assert_eq!(e.input.as_str(), "**make me bold**", "the markers are not around it");
    e.sync();
    let r = e.preview.rich();
    let weights: Vec<f32> = (0..r.spans().len()).map(|k| r.spans()[k].weight).collect();
    assert_eq!(weights, vec![700.0, 700.0, 700.0], "the preview did not render it bold");
    assert_eq!(r.span_str(0), "make", "the markers leaked into the rendered text");
}

/// With nothing selected it still writes — an empty pair at the caret,
/// which is what every Markdown editor does and what lets a person type
/// into it.
#[test]
fn bold_with_no_selection_opens_a_pair() {
    let mut e = RichTextEditor::new("");
    assert!(e.bold(), "bold at an empty caret wrote nothing");
    assert_eq!(e.input.as_str(), "****");
}

#[test]
fn the_editor_is_three_nodes() {
    assert_eq!(
        <RichTextEditor as Widget>::NODE_COUNT,
        3,
        "itself, the area and the preview — a composite whose count grew \
         is a composite that spliced something"
    );
}

// ---- PaintBox --------------------------------------------------------

fn draw_a_box(out: &mut dyn lumen_static::Painter, r: [f32; 4]) {
    // Deliberately out of bounds on the right: the widget must clip it.
    out.rect([r[0], r[1], r[2] + 500.0, r[3]], lumen_static::Color(255, 0, 0, 255));
}

/// **The callback runs, and it is clipped.** Both halves: a widget that
/// never called it would draw nothing, and one that called it without a
/// clip would let the application paint over its neighbours — which is
/// exactly what a `Tree` did until T27.11, in code this repository owns.
#[test]
fn a_paintbox_runs_its_callback_inside_a_clip() {
    use lumen_static::display::{Cmd, DisplayList};

    let mut p = PaintBox::new(draw_a_box);
    let st = lay(&mut p, 200.0);
    assert!(st[0].size.w > 0.0 && st[0].size.h > 0.0, "setup: the box has no area");
    let mut list = DisplayList::new();
    Node::paint(&p, &st, 0, &mut list);

    let mut stack: Vec<[f32; 4]> = Vec::new();
    let mut ran = false;
    for c in &list.cmds {
        match c {
            Cmd::PushClip { rect } => stack.push(*rect),
            Cmd::PopClip => {
                stack.pop();
            }
            Cmd::Rect { color, rect } if *color == lumen_static::Color(255, 0, 0, 255) => {
                ran = true;
                assert!(
                    stack.iter().any(|c| c[2] <= st[0].size.w + 0.5),
                    "the callback's {rect:?} was recorded with no clip bounding it to the \
                     widget's own {:.0} px",
                    st[0].size.w
                );
            }
            _ => {}
        }
    }
    assert!(ran, "the callback never ran: nothing it drew is in the list");
}

#[test]
fn a_paintbox_that_draws_nothing_is_not_an_error() {
    use lumen_static::display::DisplayList;
    let mut p = PaintBox::default();
    let st = lay(&mut p, 200.0);
    let mut list = DisplayList::new();
    Node::paint(&p, &st, 0, &mut list);
    assert!(!list.cmds.is_empty(), "its own background is still painted");
}

/// **Emphasis followed by punctuation** — the ordinary case, and the one
/// that was broken behind sixteen green tests until the demo was
/// rendered. `**bold**,` ends with a comma, so a parser that strips
/// markers off the word's *ends* never finds the closing pair: the
/// markers render, and every word after it stays bold to the end of the
/// paragraph.
#[test]
fn a_marker_closes_even_with_punctuation_after_it() {
    let out = spans_of("Headings, **bold**, `code` and more");
    let text: Vec<&str> = out.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(
        text,
        vec!["Headings,", "bold", ",", "code", "and", "more"],
        "the markers are still in the rendered text, or the comma was eaten"
    );
    assert_eq!(out[1].1, 700.0, "`**bold**,` is bold");
    assert_eq!(out[2].1, 400.0, "its comma is not");
    assert_eq!(out[4].1, 400.0, "and bold did not leak past it");
    assert!(out[3].2, "`` `code` `` is code");
    assert!(!out[4].2, "and code did not leak either");
}

/// A marker in the middle of a word toggles there. `a**b**c` is one
/// word and three spans, and none of them carries a gap.
#[test]
fn a_marker_inside_a_word_splits_it_without_a_gap() {
    let m = Markdown::new("a**b**c next");
    let r = m.rich();
    let text: Vec<&str> = (0..r.spans().len()).map(|k| r.span_str(k)).collect();
    assert_eq!(text, vec!["a", "b", "c", "next"]);
    assert_eq!(r.spans()[1].weight, 700.0, "the middle is bold");
    assert!(!r.spans()[0].space, "a gap inside one word renders as `a b c`");
    assert!(!r.spans()[1].space);
    assert!(r.spans()[2].space, "and the word's last segment keeps its gap");
}
