//! **The measured box holds what the painter draws** — M45/T45.0 item 1,
//! for R10-F1 (`docs/plan-text-correctness.md` §1.1).
//!
//! `FontStack` measured a run's width as `max(g.x + max(advance, ink
//! width))`, and fontdue's `g.x` already includes the glyph's left
//! bearing — so a run ending in a negative-bearing glyph (`j`) measured
//! short, and the painter, which wraps inside the measured box, moved the
//! last glyph to a second line below it. A second, smaller mechanism: a
//! logical width times a fractional scale rounds down in `f32`, and the
//! painter wraps inside that too.
//!
//! §0 rule 1: measured with the measurer a window uses (`FontMetrics`)
//! and painted with the painter a window uses (`Canvas::with_font`),
//! **through a `Host` label**, so the box is the one layout gave and the
//! rect is the one paint emitted — at every scale, since the physical
//! rect is where the round-down lives. A test that measured and painted
//! with two stand-ins would prove the stand-ins agree.
//!
//! Two quantities per case, each printed beside its bound:
//! - **wrapped**: the painter's own layout of the run inside the rect it
//!   was handed differs from its layout with no width limit. Asked of
//!   `FontStack::place_run`, the function both backends paint from.
//! - **overhang**: how far the canvas's ink reaches past the text rect's
//!   right edge, top or bottom, in physical pixels. A wrapped run's
//!   second line is the large case; a glyph whose ink passes the box is
//!   the small one. Ink *left* of the rect — a leading glyph's negative
//!   bearing — is printed and not bounded: the box starts at the pen, as
//!   a CSS content box does, and a label at x = 0 could not show it at
//!   all (the canvas edge clips it), which is why the corpus starts some
//!   labels off the pixel grid (`LEADS`).
//!
//! **A cut label fits its box** — M80/T80.3 (`docs/plan-text-fit.md`,
//! "fits its box"), the second half of this file: a `Label` under
//! `TextOverflow::Ellipsis` swept over fixed widths, its painted ink held
//! to the content box and its list ending in the `…` run; and the carried
//! T80.2 defect's gate, a fixed-width label measured against its own width
//! less its padding rather than the width it was offered.
#![cfg(feature = "truetype")]

use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Cmd, Color, StaticTree};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Fixed, Label};

#[path = "fonts/pinned.rs"]
mod pinned;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Ui {
    /// A lead-in of a fractional logical width, so the label's rect
    /// starts off the pixel grid: its two edges are scaled separately,
    /// and their difference is where a width rounds down (see `LEADS`).
    #[widget_id("lead")]
    lead: Fixed,
    #[widget_id("label")]
    label: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Page {
    #[tree]
    ui: Ui,
}

/// Twelve strings. `Taj`, `Tj`, `fj` end in a negative-bearing `j`;
/// `Settings` bold at 13 px and 1.5 is the fractional-scale case the
/// review measured; the rest are controls that were right before.
pub const STRINGS: [&str; 12] =
    ["Taj", "Tj", "fj", "Settings", "jejunj", "Hij", "POSITIVE", "Wolf", "gypsy", "quay j", "Taj Mahal", "W"];
pub const SIZES: [f32; 10] = [10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0, 32.0];
pub const WEIGHTS: [f32; 2] = [400.0, 700.0];
pub const SCALES: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 2.5];

/// Where the label starts, logically. **Zero alone could not exhibit the
/// round-down**: a rect at x = 0 has a physical width of `w × k`, which
/// for these scales and whole-pixel widths is exact, and the painter's
/// slack could be deleted with every case green. Off the grid, the two
/// edges round separately.
pub const LEADS: [f32; 4] = [0.0, 0.3, 7.7, 13.1];

/// Logical window; physical is this times the scale.
const LW: f32 = 320.0;
const LH: f32 = 80.0;

pub struct Case {
    pub text: &'static str,
    pub px: f32,
    pub weight: f32,
    pub scale: f32,
    pub wrapped: bool,
    /// Physical px of ink past the rect's right edge and past its top or
    /// bottom, (x, y); 0 when inside.
    pub over: (f32, f32),
    /// Physical px of ink left of the rect: a leading glyph's negative
    /// bearing (`j`, `T`), which the box does not claim — as a CSS
    /// content box does not — and which the pen-based width is not about.
    /// Printed, not bounded.
    pub left: f32,
    /// A hash of the canvas, for "pixel-identical" comparisons.
    pub hash: u64,
}

fn stack() -> FontStack {
    FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses")
}

/// One label, laid out and painted at one (text, size, weight, scale).
fn case(font: &FontStack, text: &'static str, px: f32, weight: f32, scale: f32, lead: f32) -> Case {
    let mut app = Page::default();
    app.ui.label.set(text.to_owned());
    // No height: it paints its colour, and its ink is not the label's.
    app.ui.lead = Fixed { w: lead, h: 0.0 };
    let (pw, ph) = ((LW * scale) as u32, (LH * scale) as u32);
    let mut h: Host<Page> = Host::new(app, pw, ph);
    h.set_scale(scale);
    h.set_stylesheet(&[Rule::new(
        Selector::Id("label"),
        Patch::new().font_size(px).font_weight(weight).color(Color(0, 0, 0, 255)),
    )]);
    let mut m = FontMetrics::with_scale(font.clone(), scale);
    h.pump_with(&mut m);

    // The rect paint emitted, in physical pixels (the list is scaled).
    let (rect, size, w) = h
        .list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Text { rect, text: t, size, weight, .. } if h.list.str_of(*t) == text => {
                Some((*rect, *size, *weight))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("setup: no text command for {text:?}"));

    // Wrapped: the painter's layout inside the rect against its layout
    // with no limit.
    let (inside, _) = font.place_run(text, size, w, rect[2] - rect[0]);
    let (free, _) = font.place_run(text, size, w, f32::INFINITY);
    let wrapped = inside.len() != free.len()
        || inside.iter().zip(&free).any(|(a, b)| a.x != b.x || a.y != b.y);

    let mut canvas = Canvas::with_font(pw, ph, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    h.render(&mut canvas);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut hash: u64 = 0xcbf29ce484222325;
    for y in 0..ph {
        for x in 0..pw {
            let i = ((y * pw + x) * 4) as usize;
            let p = &canvas.pixels[i..i + 3];
            for b in p {
                hash = (hash ^ *b as u64).wrapping_mul(0x100000001b3);
            }
            if p.iter().any(|&c| c != 255) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    assert!(x1 > 0, "setup: {text:?} at {px}/{weight}/{scale} painted no ink");
    let left = (rect[0] - x0 as f32).max(0.0);
    let ox = (x1 as f32 - rect[2]).max(0.0);
    let oy = (rect[1] - y0 as f32).max(y1 as f32 - rect[3]).max(0.0);
    Case { text, px, weight, scale, wrapped, over: (ox, oy), left, hash }
}

/// Every case, in a fixed order.
pub fn corpus(font: &FontStack) -> Vec<Case> {
    let mut out = Vec::with_capacity(1200);
    let mut n = 0;
    for &t in &STRINGS {
        for &px in &SIZES {
            for &w in &WEIGHTS {
                for &k in &SCALES {
                    out.push(case(font, t, px, w, k, LEADS[n % LEADS.len()]));
                    n += 1;
                }
            }
        }
    }
    out
}

#[test]
fn every_measured_run_holds_its_painted_ink() {
    let font = stack();
    let all = corpus(&font);
    let wrapped: Vec<&Case> = all.iter().filter(|c| c.wrapped).collect();
    let worst_x = all.iter().map(|c| c.over.0).fold(0.0, f32::max);
    let worst_y = all.iter().map(|c| c.over.1).fold(0.0, f32::max);
    let worst_left = all.iter().map(|c| c.left).fold(0.0, f32::max);
    let outside: Vec<&Case> = all.iter().filter(|c| c.over.0 > 1.0 || c.over.1 > 1.0).collect();
    for c in wrapped.iter().chain(outside.iter()).take(12) {
        println!(
            "  {:?} {} px w{} x{}: wrapped={} overhang x={} y={}",
            c.text, c.px, c.weight, c.scale, c.wrapped, c.over.0, c.over.1
        );
    }
    let named = all.iter().find(|c| c.text == "Settings" && c.px == 13.0 && c.weight == 700.0 && c.scale == 1.5);
    if let Some(c) = named {
        println!("Settings bold 13 px at 1.5: wrapped={} overhang {:?}", c.wrapped, c.over);
    }
    println!(
        "wrapped {} / {}, worst overhang x/y {worst_x}/{worst_y} px (bound 1.0), {} outside; leading bearing up to {worst_left} px (not bounded)",
        wrapped.len(),
        all.len(),
        outside.len()
    );
    assert_eq!(all.len(), 1200, "setup: the corpus is 12 x 10 x 2 x 5");
    assert!(wrapped.is_empty(), "{} of {} runs re-wrapped inside their measured box", wrapped.len(), all.len());
    assert!(outside.is_empty(), "{} runs painted ink more than 1 px outside their box", outside.len());
}

/// **The accept direction**: runs that were right before stay on one line
/// and inside the box.
#[test]
fn a_run_with_no_overhanging_glyph_stays_on_one_line() {
    let font = stack();
    for t in ["Taj Mahal", "W"] {
        for &k in &SCALES {
            let c = case(&font, t, 16.0, 400.0, k, 7.7);
            println!("{t:?} x{k}: wrapped={} overhang {:?} (bound 1.0)", c.wrapped, c.over);
            assert!(!c.wrapped && c.over.0 <= 1.0 && c.over.1 <= 1.0, "{t:?} at {k}");
        }
    }
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct FieldUi {
    #[widget_id("field")]
    field: lumen_static_widgets::TextInput,
}

#[derive(DeriveApp, Default)]
struct FieldPage {
    #[tree]
    ui: FieldUi,
}

/// **A field scrolled part-way paints its run on one line** — the same
/// rule for `TextInput`, whose run is shaped unbounded and painted in a
/// rect it chooses. It handed the painter `inner + scroll`, narrower
/// than the run whenever the caret was not at the end, and the word
/// crossing the field's right edge moved to a second line under the
/// clip: the visible line ended a word early.
#[test]
fn a_part_scrolled_field_paints_its_run_on_one_line() {
    use lumen_static::{Event, Key};
    let font = stack();
    let text = "the quick brown fox jumps over the lazy dog";
    let mut app = FieldPage::default();
    app.ui.field.set_str(text);
    let mut h: Host<FieldPage> = Host::new(app, 160, 40);
    h.set_stylesheet(&[]);
    let mut m = FontMetrics::new(font.clone());
    h.pump_with(&mut m);
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump_with(&mut m);
    assert_eq!(h.input.focus, Some(1), "setup: Tab did not focus the field");
    // From the end, back to the middle: scrolled, but not to either end.
    for _ in 0..22 {
        h.post(Event::Key { key: Key::Left, down: true });
        h.pump_with(&mut m);
    }
    let (rect, size, w) = h
        .list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Text { rect, text: t, size, weight, .. } if h.list.str_of(*t) == text => Some((*rect, *size, *weight)),
            _ => None,
        })
        .expect("setup: no text command for the field");
    let s = h.states[1];
    let scrolled = rect[0] < s.pos.x + s.pad.0 - 0.5;
    let (inside, _) = font.place_run(text, size, w, rect[2] - rect[0]);
    let (free, _) = font.place_run(text, size, w, f32::INFINITY);
    let wrapped = inside.len() != free.len() || inside.iter().zip(&free).any(|(a, b)| a.y != b.y);
    println!("field scrolled {scrolled} (text x {} < edge {}), rect width {}, wrapped {wrapped} (bound false)", rect[0], s.pos.x + s.pad.0, rect[2] - rect[0]);
    assert!(scrolled, "setup: the field did not scroll, so its rect was the whole run anyway");
    assert!(!wrapped, "a part-scrolled field's run was re-wrapped inside its rect");
}

// ---- M80/T80.3: `TextOverflow::Ellipsis` fits its box ---------------------

use lumen_static::style::{Edges, Length, TextOverflow};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct CutUi {
    #[widget_id("lab")]
    lab: Label<String>,
}

#[derive(DeriveApp, Default)]
struct CutPage {
    #[tree]
    ui: CutUi,
}

/// Wider than every label width in the sweep, so the width the label is
/// offered is never its own: the room the cut is made against has to be
/// the label's (`text_room` in `ops.rs`), or the ink leaves the box.
const CUT_WINDOW: (u32, u32) = (480, 160);
const CUT_PAD: f32 = 4.0;

const CUT_TEXTS: [&str; 5] = [
    "The quick brown fox jumps over the lazy dog",
    "Supercalifragilisticexpialidocious",
    "Family \u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467} trip, flags \u{1f1f3}\u{1f1f4} and more",
    "Inbox \u{2013} twelve unread messages from the team",
    "First line\nsecond line",
];

struct CutFrame {
    /// The content box (logical = physical at scale 1).
    content: [f32; 4],
    canvas: Canvas,
    /// Every command, its text resolved, for byte-identity.
    list: Vec<(String, String)>,
    cut: bool,
}

fn cut_frame(font: &FontStack, text: &str, width: f32, pad: f32, overflow: TextOverflow) -> CutFrame {
    let mut a = CutPage::default();
    a.ui.lab.set(text.to_owned());
    let mut host: Host<CutPage> = Host::new(a, CUT_WINDOW.0, CUT_WINDOW.1);
    host.set_stylesheet(&[Rule::new(
        Selector::Id("lab"),
        Patch::new()
            .width(Length::Fixed(width))
            .padding(Edges::all(pad))
            .overflow(overflow)
            .color(Color(0, 0, 0, 255)),
    )]);
    let mut m = FontMetrics::new(font.clone());
    host.pump_with(&mut m);
    let i = lumen_static::style::index_of_wid(<CutUi as lumen_static::Widget>::SUB_TABLE, "lab").expect("#lab");
    let s = host.states[i];
    let content = [s.pos.x + pad, s.pos.y + pad, s.pos.x + s.size.w - pad, s.pos.y + s.size.h - pad];
    let mut canvas = Canvas::with_font(CUT_WINDOW.0, CUT_WINDOW.1, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    host.render(&mut canvas);
    let list = host
        .list
        .cmds
        .iter()
        .map(|c| {
            let t = match c {
                Cmd::Text { text, .. } => host.list.str_of(*text).to_owned(),
                _ => String::new(),
            };
            (format!("{c:?}"), t)
        })
        .collect();
    CutFrame { content, canvas, list, cut: host.app.ui.lab.shown().1 }
}

/// How far the painted ink reaches past the content box's right edge, top
/// or bottom, in px; 0 inside (or with no ink at all). Ink *left* of the
/// box — a leading `T`'s negative bearing — is not counted, for the reason
/// the first half of this file gives: the box starts at the pen.
fn overhang(f: &CutFrame) -> f32 {
    let c = &f.canvas;
    let (mut y0, mut x1, mut y1) = (u32::MAX, 0u32, 0u32);
    for y in 0..c.height {
        for x in 0..c.width {
            let i = ((y * c.width + x) * 4) as usize;
            if c.pixels[i..i + 3].iter().any(|&v| v != 255) {
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 == 0 {
        return 0.0;
    }
    let b = f.content;
    [b[1] - y0 as f32, x1 as f32 - b[2], y1 as f32 - b[3]].into_iter().fold(0.0f32, f32::max)
}

/// **Every truncated label inks inside its box and ends in `…`** — 46
/// widths (40–400 px in 8 px steps) over five texts. The truncated count
/// is printed against a floor of 30, and every case that was *not* cut is
/// the accept direction: its list is byte-identical to the same label
/// under `Wrap`.
#[test]
fn every_truncated_label_inks_inside_its_box_and_ends_in_an_ellipsis() {
    let font = stack();
    let (mut cases, mut truncated, mut identical, mut worst) = (0, 0, 0, 0.0f32);
    let mut wrong = Vec::new();
    for text in CUT_TEXTS {
        for width in (40..=400).step_by(8) {
            cases += 1;
            let f = cut_frame(&font, text, width as f32, CUT_PAD, TextOverflow::Ellipsis);
            if !f.cut {
                let w = cut_frame(&font, text, width as f32, CUT_PAD, TextOverflow::Wrap);
                if f.list == w.list {
                    identical += 1;
                } else {
                    wrong.push(format!("{text:?} at {width}: uncut, and its list differs from Wrap's"));
                }
                continue;
            }
            truncated += 1;
            let over = overhang(&f);
            worst = worst.max(over);
            if over > 0.0 {
                wrong.push(format!("{text:?} at {width}: ink {over} px past the content box {:?}", f.content));
            }
            let last = f.list.iter().rev().find(|(c, _)| c.starts_with("Text"));
            if last.map(|(_, t)| t.as_str()) != Some("\u{2026}") {
                wrong.push(format!("{text:?} at {width}: the last run is {last:?}, not the ellipsis"));
            }
        }
    }
    println!(
        "{cases} cases: {truncated} truncated (bound >= 30), worst ink overhang {worst} px (bound 0); \
         {identical} uncut, each byte-identical to Wrap"
    );
    assert_eq!(cases, 230, "setup: the sweep is 46 widths x 5 texts");
    assert!(truncated >= 30, "only {truncated} cases truncated -- the sweep proves little");
    assert!(identical > 0, "setup: no case fit, so the accept direction never ran");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// **The carried T80.2 defect's own gate**: a fixed-width label measures
/// against its own width less its padding, not the width it is offered.
/// `Fixed(90)` with 8 px padding in a 480 px window wraps at 74 px. The
/// renderer wraps a run at its rect, so a label that measured at the offer
/// was one line tall and painted three, past its own box.
#[test]
fn a_fixed_width_label_wraps_at_its_own_width_less_padding() {
    let font = stack();
    let text = "Hold Lid Bold Kid Held Did";
    let f = cut_frame(&font, text, 90.0, 8.0, TextOverflow::Wrap);
    let one = lumen_static::TextMeasure::measure_weighted(&mut FontMetrics::new(font.clone()), text, 16.0, 400.0, f32::INFINITY).h;
    let lines = ((f.content[3] - f.content[1]) / one).round();
    let over = overhang(&f);
    println!(
        "Fixed(90), padding 8, offered {} px: content box {:?}, {lines} lines of {one} px (setup > 1); \
         ink overhang {over} px (bound 0)",
        CUT_WINDOW.0, f.content
    );
    assert!(lines > 1.0, "the label measured one line: it measured against the width it was offered");
    assert_eq!(over, 0.0, "the label measured fewer lines than it painted: ink {over} px past its box");
}

/// **`Clip` never paints outside its node** — M80/T80.3. A clipped
/// label whose text changes damages exactly its own box (1.00×, printed),
/// because the cut is made in layout and the prefix run is handed the
/// content box. `Recording` damages a run's whole rect whatever clip is
/// pushed, so a label that painted its whole text in a rect of its natural
/// width under a clip would damage past its node; the agent's journal
/// cannot see that (it signs nodes, not runs), which is why this asks the
/// host's own retained damage.
#[test]
fn a_clipped_labels_change_damages_only_its_box() {
    let font = stack();
    let mut a = CutPage::default();
    a.ui.lab.set("a first label far too long for its box".to_owned());
    let mut host: Host<CutPage> = Host::new(a, CUT_WINDOW.0, CUT_WINDOW.1);
    host.set_stylesheet(&[Rule::new(
        Selector::Id("lab"),
        Patch::new().width(Length::Fixed(80.0)).overflow(TextOverflow::Clip).color(Color(0, 0, 0, 255)),
    )]);
    let mut m = FontMetrics::new(font.clone());
    host.pump_with(&mut m);
    let before = host.app.ui.lab.shown().0.to_owned();
    host.app.ui.lab.set_str("another first label far too long for its box");
    assert!(host.invalidate("lab"), "setup: #lab is not addressable");
    host.pump_with(&mut m);
    let i = lumen_static::style::index_of_wid(<CutUi as lumen_static::Widget>::SUB_TABLE, "lab").expect("#lab");
    let s = host.states[i];
    let view = [0.0, 0.0, CUT_WINDOW.0 as f32, CUT_WINDOW.1 as f32];
    let regions: Vec<_> = host.damage().regions(view).collect();
    let damaged: f32 = regions.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum();
    let own = s.size.w * s.size.h;
    println!(
        "clipped label {:?}+{:?}, cut {before:?} -> {:?}: damage {damaged} px2 in {} region(s) against its box \
         {own} px2 = {:.2}x (bound 1.00x)",
        s.pos,
        s.size,
        host.app.ui.lab.shown(),
        regions.len(),
        damaged / own.max(1.0)
    );
    assert!(host.app.ui.lab.shown().1, "setup: the label is not cut");
    assert_ne!(before, host.app.ui.lab.shown().0, "setup: the cut did not change, so nothing was repainted");
    assert!(damaged > 0.0, "setup: the change damaged nothing");
    assert!((damaged / own - 1.0).abs() < 0.005, "a clipped label damaged {:.2}x its own box", damaged / own);
}
