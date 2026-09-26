//! M27/T27.11 — the four pickers.
//!
//! All four are a trigger and a popup of cells, so most of what is
//! asserted here is asserted once: the grid arithmetic, the press that
//! opens, the cell that picks and closes. What differs per kind is the
//! *content* of a cell, and that is where the date arithmetic lives —
//! the only real computation in the tranche, and the only place a
//! widget can be wrong in a way nobody notices for a month.

use lumen_static::event::{Event, EventCtx};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::pick::{days_in_month, is_leap, weekday_of, Pad, COLOR, DATE, TIME};
use lumen_static_widgets::pick::FILE;
use lumen_static_widgets::{Picker, MAX_FILES, SWATCHES};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

fn lay<W: Widget + Node>(w: &mut W) -> Vec<NodeState> {
    let styles = lumen_static::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

// ---- the date arithmetic ---------------------------------------------

/// **Against dates somebody can check**, not against the formula that
/// produced them. Sakamoto's method is short enough to retype wrong.
#[test]
fn the_weekday_is_the_one_the_calendar_says() {
    // 0 = Monday.
    for (y, m, d, want, what) in [
        (2026, 9, 19, 5u8, "the day this was written, a Saturday"),
        (2026, 1, 1, 3, "New Year 2026, a Thursday"),
        (2000, 2, 29, 1, "a leap day, a Tuesday"),
        (1970, 1, 1, 3, "the epoch, a Thursday"),
        (2024, 12, 25, 2, "Christmas 2024, a Wednesday"),
    ] {
        assert_eq!(weekday_of(y, m, d), want, "{what}: {y}-{m:02}-{d:02}");
    }
}

#[test]
fn february_knows_about_leap_years() {
    assert_eq!(days_in_month(2024, 2), 29, "2024 is a leap year");
    assert_eq!(days_in_month(2026, 2), 28, "2026 is not");
    assert_eq!(days_in_month(1900, 2), 28, "1900 is a century and not a leap year");
    assert_eq!(days_in_month(2000, 2), 29, "2000 is a fourth century and is");
    assert!(!is_leap(1900) && is_leap(2000) && is_leap(2024) && !is_leap(2026));
    // And the ordinary months, so a transposed table is caught.
    assert_eq!(days_in_month(2026, 1), 31);
    assert_eq!(days_in_month(2026, 4), 30);
    assert_eq!(days_in_month(2026, 12), 31);
}

/// The grid's lead-in is the first of the month's weekday, and the days
/// run from there.
#[test]
fn a_month_grid_starts_where_the_month_does() {
    let mut p = Pad::<DATE>::default();
    p.year = 2026;
    p.month = 9;
    // 2026-09-01 is a Tuesday, so one blank then the 1st.
    assert_eq!(weekday_of(2026, 9, 1), 1, "setup: September 2026 starts on a Tuesday");
    assert_eq!(p.day_at(0), None, "cell 0 is the Monday before the 1st");
    assert_eq!(p.day_at(1), Some(1), "cell 1 is the 1st");
    assert_eq!(p.day_at(30), Some(30), "cell 30 is the 30th");
    assert_eq!(p.day_at(31), None, "September has thirty days");
    assert_eq!(p.day_at(41), None, "and the grid's last cell is blank");
}

/// **Paging moves the month on show, never the chosen date** — M57/B17.
///
/// This test used to assert the defect as the design: paging from 31
/// January moved the *chosen* date to 28 February. One press on `›`
/// changed what the trigger read, with no day picked
/// (`docs/widget-audit.md`, DatePicker).
#[test]
fn paging_moves_the_view_and_not_the_chosen_date() {
    let mut p = Pad::<DATE>::default();
    p.year = 2026;
    p.month = 1;
    p.day = 31;
    p.shift_month(1);
    assert_eq!((p.year, p.month, p.day), (2026, 1, 31), "paging changed the chosen date");
    assert_eq!(p.shown_month(), (2026, 2), "the view did not move to February");
    p.shift_month(-1);
    assert_eq!(p.shown_month(), (2026, 1), "and back");
    // Across both year boundaries, on the view.
    p.shift_month(11);
    assert_eq!(p.shown_month(), (2026, 12), "January + 11 is December");
    p.shift_month(1);
    assert_eq!(p.shown_month(), (2027, 1), "December + 1 is next January");
    p.shift_month(-1);
    assert_eq!(p.shown_month(), (2026, 12), "and January - 1 is last December");
    // A day picked in the shown month commits both.
    let k = (0..42).find(|&k| p.day_at(k) == Some(5)).unwrap();
    assert!(p.is_selected(k) == false, "a day in another month is not shown as chosen");
}

// ---- the grid, which every kind shares --------------------------------

#[test]
fn every_kind_lays_its_cells_out_the_same_way() {
    for (cols, cells, rows, what) in [
        (Pad::<COLOR>::default().cols(), Pad::<COLOR>::default().cells(), 6, "colour"),
        (Pad::<DATE>::default().cols(), Pad::<DATE>::default().cells(), 6, "date"),
        (Pad::<TIME>::default().cols(), Pad::<TIME>::default().cells(), 6, "time"),
    ] {
        assert_eq!(cells.div_ceil(cols), rows, "{what}: {cells} cells in {cols} columns");
    }
    assert_eq!(Pad::<COLOR>::default().cells(), SWATCHES.len(), "a swatch per colour");
}

/// A press in the header is **not** a press in row 0 — the arithmetic
/// that folded it in would page the month *and* pick a day.
#[test]
fn the_month_header_is_not_a_cell() {
    let mut p = Pad::<DATE>::default();
    let st = lay(&mut p);
    assert!(st[0].size.h > 0.0, "setup: the pad has a box");
    fn press<const K: u8>(p: &mut Pad<K>, x: f32, y: f32) -> bool {
        let ev = Event::PointerUp { x, y, button: lumen_static::PointerButton::Left };
        let mut cx = EventCtx::for_event(0, ev);
        cx.local = Point { x, y };
        Node::on_event(p, &ev, &cx)
    }
    let (y0, m0) = p.shown_month();
    // On the header's right arrow: the month on show pages and no day is
    // picked. (Its middle is the title, which pages nothing — M57/B17.)
    let day = p.day;
    let right = st[0].size.w - 12.0;
    assert!(press(&mut p, right, 8.0 + 4.0), "a press in the header did nothing");
    assert_eq!(p.day, day, "paging the month also picked a day");
    assert!(
        p.shown_month() != (y0, m0),
        "the header press did not page the month: still {y0}-{m0:02}"
    );
    assert_eq!((p.year, p.month), (y0, m0), "paging changed the chosen month");
    // The title between the arrows pages nothing.
    let before = p.shown_month();
    press(&mut p, st[0].size.w / 2.0, 8.0 + 4.0);
    assert_eq!(p.shown_month(), before, "a press on the title paged the month");

    // **And the row below it is row 0.** The assertion above cannot see
    // a grid that forgot to skip the header — the paging arm returns
    // before the cell arithmetic is reached, so an injection that
    // dropped `head_h` from `cell_at` left it green. This is the half
    // that measures: September 2026 starts on a Tuesday, so the first
    // week's Wednesday is the 2nd, and a grid that started its rows at
    // the pad's top would answer with the 9th.
    p.year = 2026;
    p.month = 9;
    // Back to showing the chosen month: the press above paged the view.
    p.view = None;
    // From the widget, not from a literal: the strip of weekday
    // headings is part of the head and a test with `28.0` in it
    // would start pressing the wrong row the day one was added.
    let row0_y = 8.0 + p.head_h() + 26.0 / 2.0;
    assert!(press(&mut p, 8.0 + 30.0 * 2.5, row0_y), "a press in the first week did nothing");
    assert_eq!(
        p.day, 2,
        "the first week's Wednesday is the 2nd; row 0 was read as row {}",
        (p.day.max(1) - 1) / 7
    );

    // A blank lead-in cell is not a day, and a press on it takes
    // nothing — including the popup's dismissal, which is what `false`
    // means to the caller.
    let before = p.day;
    assert!(
        !press(&mut p, 8.0 + 30.0 * 0.5, row0_y),
        "a press on the Monday before the 1st was taken"
    );
    assert_eq!(p.day, before, "a blank cell changed the day to {}", p.day);
}

// ---- the picker ------------------------------------------------------

fn press_at<W: Widget + Node>(w: &mut W, node: u32, x: f32, y: f32) -> bool {
    let ev = Event::PointerUp { x, y, button: lumen_static::PointerButton::Left };
    let mut cx = EventCtx::for_event(node, ev);
    cx.local = Point { x, y };
    Node::on_event(w, &ev, &cx)
}

#[test]
fn a_press_on_the_trigger_opens_it_and_another_closes_it() {
    let mut p = Picker::<COLOR>::new();
    lay(&mut p);
    assert!(!p.is_open(), "setup: a picker starts closed");
    assert!(press_at(&mut p, 1, 10.0, 10.0), "the trigger declined a press");
    assert!(p.is_open(), "the press did not open the popup");
    assert!(press_at(&mut p, 1, 10.0, 10.0), "the second press was declined");
    assert!(!p.is_open(), "the second press did not close it");
}

/// Escape closes it — which is what `dismiss` is for, and what a
/// keyboard user has instead of a press outside.
#[test]
fn escape_closes_it() {
    let mut p = Picker::<DATE>::new();
    lay(&mut p);
    press_at(&mut p, 1, 10.0, 10.0);
    assert!(p.is_open(), "setup: it did not open");
    let ev = Event::Key { key: lumen_static::event::Key::Escape, down: true };
    assert!(
        Node::on_event(&mut p, &ev, &EventCtx::for_event(0, ev)),
        "Escape was declined while the popup was open"
    );
    assert!(!p.is_open(), "Escape did not close it");
}

/// **Picking a cell closes the popup**, and the close happens in
/// `layout` — where `&mut self` and the state array are both in hand,
/// because the node the press reached is the *pad*, not the picker.
#[test]
fn picking_a_cell_sets_the_value_and_closes_the_popup() {
    let mut p = Picker::<COLOR>::new();
    lay(&mut p);
    press_at(&mut p, 1, 10.0, 10.0);
    assert!(p.is_open(), "setup: it did not open");
    let before = p.text();
    // Row 1, column 1 of the swatch grid — cell 5, the mid red. The
    // *index* is asserted through the colour it names, so a change to
    // the grid's shape cannot quietly move what this presses.
    let (cw, ch) = (32.0f32, 26.0f32);
    let ev = Event::PointerUp { x: 0.0, y: 0.0, button: lumen_static::PointerButton::Left };
    let mut cx = EventCtx::for_event(3, ev);
    cx.local = Point { x: 8.0 + cw * 1.5, y: 8.0 + ch * 1.5 };
    assert!(
        Node::on_event(p.cells_mut(), &ev, &cx),
        "the pad declined a press on one of its cells"
    );
    // The picker takes the choice on its next layout.
    lay(&mut p);
    assert!(!p.is_open(), "picking a swatch left the popup open");
    assert_ne!(p.text(), before, "the trigger still reads {before}");
    assert_eq!(p.text(), "#dc2626", "row 1 column 1 is the mid red");
}

#[test]
fn each_kind_reads_its_own_value() {
    let mut c = Picker::<COLOR>::new();
    assert_eq!(c.text(), "#1f2937", "a colour reads as a hex triple");
    let mut d = Picker::<DATE>::new();
    d.cells_mut().year = 2026;
    d.cells_mut().month = 9;
    d.cells_mut().day = 7;
    assert_eq!(d.text(), "2026-09-07", "a date is ISO, and zero-padded");
    let mut t = Picker::<TIME>::new();
    t.cells_mut().hour = 14;
    t.cells_mut().minute = 5;
    assert_eq!(t.text(), "14:05", "a time is 24-hour and zero-padded");
    let f = Picker::<FILE>::new().with_files(&["a.rs", "b.rs"]);
    assert_eq!(f.text(), "a.rs", "a file picker reads the name it is on");
    let _ = (&mut c, &mut d, &mut t);
}

/// A file picker takes the names it is given, and truncates rather than
/// refusing: a picker that showed nothing because the directory was
/// large is worse than one that shows the first sixty-four.
#[test]
fn a_file_list_truncates_rather_than_refusing() {
    let many: Vec<&'static str> = core::iter::repeat_n("x.rs", MAX_FILES + 10).collect();
    let f = Picker::<FILE>::new().with_files(&many);
    assert_eq!(
        f.cells().cells(),
        MAX_FILES,
        "a list longer than the cap should be cut to it, not dropped"
    );
}
