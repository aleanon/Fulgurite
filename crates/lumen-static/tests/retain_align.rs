//! M26/T26.11: a command-count change is no longer `Damage::All`.
//!
//! `Incremental` compared slot-by-slot *during* the walk, overwriting
//! each slot as it went — so by the time `finish` knew the lengths
//! differed, the previous frame was gone and the only honest answer was
//! the whole window. Every popup open and close therefore repainted
//! everything, on the strategy the desktop shell defaults to.
//!
//! Two buffers now, aligned by longest common prefix and suffix over
//! the hashes, damaging the middle.
//!
//! **The good-input half is stated as area, not as a variant.** After
//! this change a frame in which every command differs reports *regions*
//! covering the viewport rather than `Damage::All`, so the assertions
//! compare covered area against the viewport's and print both.

use lumen_static::display::{Color, Painter, Rect};
use lumen_static::retain::{Damage, Incremental, Recording, Retention};

const VIEW: Rect = [0.0, 0.0, 400.0, 300.0];

/// A sink that keeps nothing: this test is about the *retention*, and a
/// display list would only add a second thing to get wrong.
#[derive(Default)]
struct Null;

impl Painter for Null {
    // M82/T82.3: a leaf with no fill table.
    fn fills(&self) -> &[lumen_static::style::Fill] {
        &[]
    }
    fn rect(&mut self, _: Rect, _: Color) {}
    fn text_spaced(&mut self, _: Rect, _: &str, _: f32, _: f32, _: u8, _: Color) {}
    fn rounded_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn shadow(&mut self, _: Rect, _: f32, _: f32) {}
    fn stroked_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn path(&mut self, _: &[lumen_static::display::PathEl], _: lumen_static::display::Affine, _: lumen_static::display::PathPaint) {}
}

/// Paint one frame of `rects` and return its damage.
fn frame(ret: &mut Incremental, rects: &[Rect]) -> Damage {
    let mut null = Null;
    {
        let mut rec = Recording::new(&mut null, ret);
        for r in rects {
            // **The colour comes from the rect, not from the index.**
            // Colouring by position makes every command after an
            // insertion differ, so the aligner's suffix match never
            // fires and the test measures its own fixture: the first
            // version reported 11,200 px² for a 3,200 px² popup and the
            // extra was the chrome below it, re-coloured.
            rec.rect(*r, Color(r[0] as u8, r[1] as u8, r[2] as u8, 255));
        }
    }
    ret.finish()
}

fn area(d: &Damage) -> f32 {
    d.regions(VIEW).map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum()
}

const VIEW_AREA: f32 = 400.0 * 300.0;

/// The window's chrome, unchanged between frames.
fn chrome() -> Vec<Rect> {
    vec![[0.0, 0.0, 400.0, 20.0], [0.0, 280.0, 400.0, 300.0]]
}

/// **A popup opens: the damage is the popup, not the window.**
#[test]
fn a_frame_that_grows_damages_only_what_appeared() {
    let mut ret = Incremental::default();
    let base = chrome();
    frame(&mut ret, &base);
    frame(&mut ret, &base);

    let popup: Rect = [100.0, 100.0, 180.0, 140.0];
    let mut opened = base.clone();
    opened.insert(1, popup);
    let d = frame(&mut ret, &opened);

    let got = area(&d);
    let want = (popup[2] - popup[0]) * (popup[3] - popup[1]);
    println!("popup open: {got} px² damaged, the popup is {want} px², viewport {VIEW_AREA}");
    // **The bound is the popup's own rect**, not a fraction of the
    // viewport: the head and tail of the stream are untouched, so the
    // aligner has exactly one command to blame and nothing else may be
    // in the answer.
    assert_eq!(
        got, want,
        "opening a popup damaged {got} px² where the popup is {want} px² \
         (viewport {VIEW_AREA})"
    );
}

/// And closing it again.
#[test]
fn a_frame_that_shrinks_damages_only_what_left() {
    let mut ret = Incremental::default();
    let popup: Rect = [100.0, 100.0, 180.0, 140.0];
    let mut opened = chrome();
    opened.insert(1, popup);
    frame(&mut ret, &opened);
    frame(&mut ret, &opened);

    let d = frame(&mut ret, &chrome());
    let got = area(&d);
    println!("popup close: {got} px² damaged of {VIEW_AREA}");
    let want = (popup[2] - popup[0]) * (popup[3] - popup[1]);
    assert_eq!(
        got, want,
        "closing a popup damaged {got} px² where the popup is {want} px² \
         (viewport {VIEW_AREA})"
    );
}

/// **The good-input direction, as area.** A frame in which everything
/// differs must still cover the viewport — an alignment that matched
/// two unrelated streams would report far less.
#[test]
fn a_frame_that_differs_everywhere_still_covers_the_viewport() {
    let mut ret = Incremental::default();
    let a: Vec<Rect> = (0..6)
        .map(|k| [k as f32 * 60.0, 0.0, k as f32 * 60.0 + 60.0, 300.0])
        .collect();
    frame(&mut ret, &a);
    frame(&mut ret, &a);

    // Same count, every hash different (the colour is the index).
    let b: Vec<Rect> = (0..7)
        .map(|k| [k as f32 * 55.0, 0.0, k as f32 * 55.0 + 55.0, 300.0])
        .collect();
    let d = frame(&mut ret, &b);
    let got = area(&d);
    println!("all-different: {got} px² of {VIEW_AREA}");
    assert!(
        got >= VIEW_AREA * 0.9,
        "a frame that differs everywhere covered only {got} px² of \
         {VIEW_AREA} — the alignment matched streams that do not match"
    );
}

/// A still frame is still still. The alignment must not invent damage.
#[test]
fn an_unchanged_frame_damages_nothing() {
    let mut ret = Incremental::default();
    let base = chrome();
    frame(&mut ret, &base);
    frame(&mut ret, &base);
    let d = frame(&mut ret, &base);
    assert_eq!(area(&d), 0.0, "an unchanged frame reported {d:?}");
}

/// One command's content changing, with the count unchanged, is the
/// case that already worked — and must keep working, bounded to that
/// command's own rect.
#[test]
fn one_changed_command_damages_its_own_rect() {
    let mut ret = Incremental::default();
    let base = chrome();
    frame(&mut ret, &base);
    frame(&mut ret, &base);

    let mut moved = base.clone();
    moved[1] = [0.0, 280.0, 200.0, 300.0];
    let d = frame(&mut ret, &moved);
    let got = area(&d);
    println!("one command moved: {got} px²");
    assert!(got > 0.0, "a changed command reported no damage");
    assert!(
        got < VIEW_AREA * 0.2,
        "one changed command damaged {got} px² of {VIEW_AREA}"
    );
}

/// **A line pitch change is damage** — M80/T80.1.
///
/// The pitch moves every line after the first inside a rect that need not
/// change, so a `Recording` that left `line_height` out of its hash would
/// call the frame unchanged and leave the old lines on screen. One rect,
/// one string, one size, weight and colour on both frames: the byte is
/// the only difference.
#[test]
fn a_line_height_change_damages_the_run() {
    fn run(ret: &mut Incremental, rect: Rect, lh: u8) -> Damage {
        let mut null = Null;
        {
            let mut rec = Recording::new(&mut null, ret);
            for r in chrome() {
                rec.rect(r, Color(r[0] as u8, r[1] as u8, r[2] as u8, 255));
            }
            rec.text_spaced(rect, "two\nlines", 14.0, 400.0, lh, Color(0, 0, 0, 255));
        }
        ret.finish()
    }
    let (a, b): (Rect, Rect) = ([40.0, 60.0, 200.0, 120.0], [40.0, 60.0, 200.0, 120.0]);
    println!("setup: rects {a:?} and {b:?} (equal: {})", a == b);
    assert_eq!(a, b, "setup: the rects differ, so the byte is not the only difference");
    let mut ret = Incremental::default();
    run(&mut ret, a, 16);
    run(&mut ret, a, 16);
    let same = area(&run(&mut ret, a, 16));
    let d = run(&mut ret, b, 24);
    let got = area(&d);
    let want = (a[2] - a[0]) * (a[3] - a[1]);
    println!("line_height 16 -> 24: damaged {got} px² against the rect's {want} px² (bound: equal); unchanged frame {same} px²");
    assert_eq!(same, 0.0, "setup: an unchanged frame reported damage, so the baseline is not still");
    assert_eq!(got, want, "a line_height change damaged {got} px², not the run's {want} px²");
}
