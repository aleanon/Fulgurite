//! **A `LinkedMarkdown`'s links activate, through the pass** — M87/T87.3
//! (`docs/plan-markdown.md`).
//!
//! Every gate here goes through a real `Host`: `Host::click` (a press and a
//! release, queued) and `Host::pump`, whose answer is whether any event
//! was handled — the only direct answer to "did that input land", and the
//! only way to test that a click that must do nothing does nothing. A
//! link is reported by `hit` as `Inner { row: n }`, so the pass sets
//! `ctx.row = Some(n)` and asks the handler with it, and the press latch
//! compares rows.

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::prelude::*;
use lumen_static::style::{Cursor, Palette, Role};
use lumen_static::Event;
use lumen_static_shell::Host;
use lumen_static_widgets::{link_target, LinkedMarkdown, MarkdownTheme};

/// Two links on one line, and plain words around them.
const DOC: &str = "Read the [guide](https://lumen.example/guide) or the [changelog](https://lumen.example/changes) first.";

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("doc")]
    doc: LinkedMarkdown<Doc>,
}

#[derive(DeriveApp, Default)]
struct Doc {
    #[tree]
    ui: Ui,
    last_link: Option<u64>,
    runs: u32,
}

fn follow(d: &mut Doc, cx: &lumen_static::EventCtx) {
    d.last_link = cx.row;
    d.runs += 1;
}

fn host() -> Host<Doc> {
    let mut a = Doc::default();
    a.ui.doc = LinkedMarkdown::linked(DOC, follow);
    let mut h: Host<Doc> = Host::new(a, 600, 200);
    h.pump();
    h
}

/// Link `n`'s first and last words' boxes, in window coordinates:
/// `(first, last)` as `[x0, y0, x1, y1]`.
fn link_boxes(h: &Host<Doc>, n: u16) -> ([f32; 4], [f32; 4]) {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "doc").expect("#doc");
    let s = h.states[i];
    let r = h.app.ui.doc.rich();
    let line = s.font_size * 1.45;
    let boxes: Vec<[f32; 4]> = r
        .spans()
        .iter()
        .zip(r.placed())
        .filter(|(sp, _)| sp.link == n)
        .map(|(sp, p)| {
            let (x, y) = (s.pos.x + s.pad.0 + p.0, s.pos.y + s.pad.1 + p.1);
            [x, y, x + p.2, y + line * sp.scale]
        })
        .collect();
    assert!(!boxes.is_empty(), "setup: link {n} has no words");
    (boxes[0], *boxes.last().unwrap())
}

fn centre(b: [f32; 4]) -> (f32, f32) {
    ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0)
}

/// **Gate: a click on link 1 runs its handler with `ctx.row == Some(1)`**,
/// and the same click 4 px past its last word runs nothing. Injections
/// (recorded in the commit): `hit` answering `inner: None` (handled
/// false), and `hit` answering the previous link's index (`Some(0)`).
#[test]
fn a_click_on_a_link_runs_its_handler_with_its_index() {
    let mut h = host();
    assert_eq!(h.app.ui.doc.link_count(), 2, "setup: the document holds two links");
    let (first, last) = link_boxes(&h, 1);
    let (x, y) = centre(first);
    h.click(x, y);
    let handled = h.pump();
    let target = h.app.last_link.and_then(|n| link_target(DOC, n as usize));
    println!(
        "click at ({x:.1}, {y:.1}), link 1's word {first:?}: handled {handled} (bound true), last_link {:?} (bound Some(1)), \
         link_target(DOC, 1) {target:?} (bound \"https://lumen.example/changes\")",
        h.app.last_link
    );
    assert!(handled, "a click on a link was not handled");
    assert_eq!(h.app.last_link, Some(1), "the handler did not get the link's index in ctx.row");
    assert_eq!(target, Some("https://lumen.example/changes"));
    assert_eq!(h.app.ui.doc.link(1), target, "Markdown::link and link_target disagree");

    // Refuse: 4 px past the link's last word is plain text.
    let before = (h.app.last_link, h.app.runs);
    let (px, py) = (last[2] + 4.0, (last[1] + last[3]) / 2.0);
    h.click(px, py);
    let handled = h.pump();
    println!("click 4 px past it at ({px:.1}, {py:.1}): handled {handled} (bound false), state {:?} (bound {before:?})", (h.app.last_link, h.app.runs));
    assert!(!handled, "a click on plain text was handled");
    assert_eq!((h.app.last_link, h.app.runs), before, "a click on plain text ran a link's handler");
}

/// **Gate: a press on link 0 released on link 1 runs nothing.** The press
/// latch compares the release's row with the press's. Injection (recorded
/// in the commit): `hit` reporting row 0 for every link — the release then
/// completes the press and the handler runs once.
#[test]
fn a_press_on_one_link_released_on_another_runs_nothing() {
    let mut h = host();
    let (a, _) = link_boxes(&h, 0);
    let (b, _) = link_boxes(&h, 1);
    let ((ax, ay), (bx, by)) = (centre(a), centre(b));
    h.post(Event::down(ax, ay));
    h.post(Event::PointerMove { x: bx, y: by });
    h.post(Event::up(bx, by));
    h.pump();
    println!("press on link 0 at ({ax:.1}, {ay:.1}), release on link 1 at ({bx:.1}, {by:.1}): handler ran {} time(s) (bound 0)", h.app.runs);
    assert_eq!(h.app.runs, 0, "a drag from one link to another followed a link");
    // Accept: the same press and release on one link runs it.
    h.click(bx, by);
    h.pump();
    println!("press and release on link 1: ran {} time(s) (bound 1), last_link {:?}", h.app.runs, h.app.last_link);
    assert_eq!((h.app.runs, h.app.last_link), (1, Some(1)), "setup: a click on link 1 did not run it");
}

/// **Gate: the pointer is a hand over a link and the default 4 px past it.**
/// `Host::cursor` is the gate: the agent has no cursor method (M44/T44.8's
/// precedent). Injection (recorded in the commit): `cursor_at` answering
/// `None` — the hover over the link reads `Default`.
#[test]
fn the_cursor_is_a_pointer_over_a_link() {
    let mut h = host();
    let (first, last) = link_boxes(&h, 0);
    let (x, y) = centre(first);
    h.post(Event::PointerMove { x, y });
    h.pump();
    let over = h.cursor();
    let (px, py) = (last[2] + 4.0, (last[1] + last[3]) / 2.0);
    h.post(Event::PointerMove { x: px, y: py });
    h.pump();
    let past = h.cursor();
    println!("cursor over link 0 {over:?} (bound Pointer), 4 px past it {past:?} (bound Default)");
    assert_eq!(over, Cursor::Pointer, "the pointer over a link is not a hand");
    assert_eq!(past, Cursor::Default, "the pointer over plain text is a hand");
}

/// The colour of the text run that says `word`.
fn ink(list: &DisplayList, word: &str) -> Option<Color> {
    list.cmds.iter().find_map(|c| match c {
        Cmd::Text { text, color, .. } if list.str_of(*text) == word => Some(*color),
        _ => None,
    })
}

/// **Gate: a link is the link colour derived from the palette's `Text`,
/// under both palettes** (the plan's `Accent` check, changed with K87c's
/// fallback: `MarkdownTheme::link_ink`). Injection (recorded in the
/// commit): paint a link span in the node's own `fg` — the link prints the
/// body colour.
#[test]
fn a_link_is_the_link_colour_under_either_palette() {
    let mut h = host();
    for (name, palette) in [("LIGHT", Palette::LIGHT), ("DARK", Palette::DARK)] {
        h.set_palette(palette);
        h.pump();
        let text = palette.get(Role::Text);
        let want = MarkdownTheme::DEFAULT.link_ink(text);
        let (l, b) = (ink(&h.list, "guide"), ink(&h.list, "Read"));
        println!("{name}: link {l:?} (bound {want:?}), body {b:?} (bound {text:?})");
        assert_eq!(b, Some(text), "setup: under {name} the body is not the palette's Text");
        assert_eq!(l, Some(want), "under {name} the link is {l:?}, not the link colour {want:?}");
    }
    println!("size_of::<Span>() = {} B (bound 24)", std::mem::size_of::<lumen_static_widgets::Span>());
    assert!(std::mem::size_of::<lumen_static_widgets::Span>() <= 24, "Span grew past 24 B");
}

/// **Gate: Tab, Down, then Enter, follows the first link** — M44/T44.10's path:
/// Enter on a focused node with an active row runs that row's handler.
/// Injection (recorded in the commit): `active_row` answering `None` —
/// Enter runs nothing.
#[test]
fn down_then_enter_follows_the_first_link() {
    let mut h = host();
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "doc").expect("#doc") as u32;
    h.post(Event::Key { key: lumen_static::event::Key::Tab, down: true });
    h.post(Event::Key { key: lumen_static::event::Key::Tab, down: false });
    h.pump();
    println!("setup: Tab focused {:?} (bound Some({i}))", h.input.focus);
    assert_eq!(h.input.focus, Some(i), "setup: Tab did not reach the LinkedMarkdown");
    for key in [lumen_static::event::Key::Down, lumen_static::event::Key::Enter] {
        h.post(Event::Key { key, down: true });
        h.post(Event::Key { key, down: false });
        h.pump();
    }
    println!("Tab, Down, Enter: active {:?}, last_link {:?} (bound Some(0)), ran {}", h.app.ui.doc.active(), h.app.last_link, h.app.runs);
    assert_eq!(h.app.ui.doc.active(), Some(0), "Down did not make the first link active");
    assert_eq!((h.app.last_link, h.app.runs), (Some(0), 1), "Enter did not follow the active link");
}
