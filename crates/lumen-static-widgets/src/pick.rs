//! M27/T27.11 — the four pickers: colour, date, time, file.
//!
//! # One shape, four contents
//!
//! All four are a **trigger and a popup of cells**, and that is not a
//! convenience — it is what they are. A colour picker is a grid of
//! swatches; a month is a grid of seven columns; a clock is a grid of
//! hours over a grid of minutes; a file list is a grid one column wide.
//! So `Picker<KIND>` owns the trigger, the popup and the hit
//! arithmetic, and the only thing a kind decides is how many columns
//! there are, what cell `k` says, and what picking it means.
//!
//! The alternative — four types with four tables — was measured in
//! lines and rejected: `docs/plan-widget-parity.md` T27.11 records it,
//! with `Strip<KIND>` (T27.6) and `Veil<W, KIND>` (T27.7) as the two
//! precedents where the same choice was made and one where it was not
//! (T27.9's `Chooser` does not use `Repeat`, and says why).
//!
//! **The cells are painted, not spliced.** Forty-two day cells as forty-two
//! table nodes would put a month grid's *data* in the type — the thing
//! this architecture exists to keep out. A pad is one node that paints
//! its cells and hit-tests them by arithmetic, which is how `Pagination`
//! draws page numbers and `Strip` draws its items.

use alloc::string::String;

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use crate::popup::Popup;
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

/// A grid of swatches.
pub const COLOR: u8 = 0;
/// A month.
pub const DATE: u8 = 1;
/// Hours over minutes.
pub const TIME: u8 = 2;
/// A list of names the application supplies.
pub const FILE: u8 = 3;

/// The swatches a `ColorPicker` offers: six hues, four lightnesses
/// each, one hue per row.
///
/// **A fixed palette, not a colour wheel.** A wheel needs a continuous
/// hit test, a saturation/value square and a hue strip — three more
/// geometries for a widget whose job in nine cases out of ten is "pick
/// one of the ones we use". The wheel is the extension, not the widget.
pub const SWATCHES: [Color; 24] = [
    Color(0x1f, 0x29, 0x37, 0xff), Color(0x4b, 0x55, 0x63, 0xff),
    Color(0x9c, 0xa3, 0xaf, 0xff), Color(0xe5, 0xe7, 0xeb, 0xff),
    Color(0x7f, 0x1d, 0x1d, 0xff), Color(0xdc, 0x26, 0x26, 0xff),
    Color(0xf8, 0x71, 0x71, 0xff), Color(0xfe, 0xca, 0xca, 0xff),
    Color(0x78, 0x35, 0x0f, 0xff), Color(0xea, 0x58, 0x0c, 0xff),
    Color(0xfb, 0x92, 0x3c, 0xff), Color(0xfe, 0xd7, 0xaa, 0xff),
    Color(0x14, 0x53, 0x2d, 0xff), Color(0x16, 0xa3, 0x4a, 0xff),
    Color(0x4a, 0xde, 0x80, 0xff), Color(0xbb, 0xf7, 0xd0, 0xff),
    Color(0x1e, 0x3a, 0x8a, 0xff), Color(0x25, 0x63, 0xeb, 0xff),
    Color(0x60, 0xa5, 0xfa, 0xff), Color(0xbf, 0xdb, 0xfe, 0xff),
    Color(0x4c, 0x1d, 0x95, 0xff), Color(0x7c, 0x3a, 0xed, 0xff),
    Color(0xa7, 0x8b, 0xfa, 0xff), Color(0xdd, 0xd6, 0xfe, 0xff),
];

/// The most names a `FilePicker` lists.
pub const MAX_FILES: usize = 64;

/// A cell's height, every kind.
const CELL_H: f32 = 26.0;
/// The month header's height — `< Sep 2026 >`.
const HEAD_H: f32 = 28.0;
/// How far in from each end of the date header a press pages the month —
/// twice the arrow's drawn width, so it is a target and not a glyph
/// (M57/B17).
const ARROW_HIT: f32 = 28.0;
/// Rows a file list shows before it scrolls — M57/B19. A long list was a
/// popup taller than the window.
pub const FILE_ROWS: usize = 8;

/// The weekday strip under it.
///
/// **A calendar without weekday headings is a grid of numbers.** The
/// first render of this had none, and the only way to tell which column
/// was Monday was to count from a date you already knew.
const DOW_H: f32 = 20.0;
/// Monday first, which is what the weekday arithmetic answers and what
/// every calendar outside North America shows.
const DOW: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

/// Days in `month` of `year`, 1-based month.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// The proleptic Gregorian leap rule.
pub fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Weekday of `year-month-day`, 0 = Monday.
///
/// Sakamoto's method, which is the whole of the date arithmetic this
/// widget needs: a month grid is a lead-in of blanks and then the days.
/// Spelled out rather than pulled in, because `chrono` is `std` and this
/// crate runs on a microcontroller.
pub fn weekday_of(year: i32, month: u8, day: u8) -> u8 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let m = month.clamp(1, 12) as usize;
    let y = if m < 3 { year - 1 } else { year };
    let sunday_based =
        (y + y / 4 - y / 100 + y / 400 + T[m - 1] + day as i32).rem_euclid(7);
    // Sakamoto answers Sunday = 0; a month grid that starts on Monday is
    // what every calendar in Europe shows, so shift once here rather
    // than at every use.
    ((sunday_based + 6) % 7) as u8
}

// ---- the pad ---------------------------------------------------------

/// The popup's contents: a grid of cells, one node.
pub struct Pad<const KIND: u8> {
    /// `COLOR`: which swatch.
    pub color: usize,
    /// `DATE`: the **chosen** date.
    pub year: i32,
    pub month: u8,
    pub day: u8,
    /// `DATE`: the month **on show**, when it is not the chosen one —
    /// M57/B17 (`docs/plan-widget-fixes.md`). Paging moved `year`/`month`,
    /// which the trigger reads: one press on `›` changed the chosen date
    /// from 2026-09-19 to 2026-10-19, and clamped a 31st to a 28th, without
    /// a day being picked (`docs/widget-audit.md`, DatePicker). The view is
    /// separate now, and only a day press commits.
    pub view: Option<(i32, u8)>,
    /// `TIME`.
    pub hour: u8,
    pub minute: u8,
    /// `FILE`: the names, and which one.
    files: [&'static str; MAX_FILES],
    file_count: usize,
    pub file: usize,
    /// `FILE`: the first file on show, moved by the wheel (M57/B19).
    first: usize,
    /// The cell under the pointer, for the hover shade.
    hot: Option<usize>,
    /// Set when a cell was chosen; the `Picker` takes it in `layout`,
    /// the way `Chooser` takes `OptionList::picked` (T27.9).
    picked: bool,
    /// Laid-out size and padding, so `on_event` can turn a local point
    /// into a cell. **Read from the style at layout, not assumed**: a
    /// sheet can set the padding, and a hit test that used a literal
    /// would pick the wrong cell for every styled pad.
    size: Size,
    pad: Edges,
    /// Widths of a one- and a two-digit label, measured at layout, so the
    /// grid can centre numbers in their cells (M57/B17: they sat 4 px in
    /// from the left, a ragged column under each weekday heading).
    digit_w: (f32, f32),
}

impl<const KIND: u8> Default for Pad<KIND> {
    fn default() -> Self {
        Pad {
            color: 0,
            year: 2026,
            month: 9,
            day: 19,
            view: None,
            hour: 9,
            minute: 0,
            files: [""; MAX_FILES],
            file_count: 0,
            file: 0,
            first: 0,
            hot: None,
            picked: false,
            digit_w: (0.0, 0.0),
            size: Size { w: 0.0, h: 0.0 },
            pad: Edges::all(8.0),
        }
    }
}

impl<const KIND: u8> Pad<KIND> {
    /// Columns in the grid. One number decides the whole geometry.
    pub fn cols(&self) -> usize {
        match KIND {
            // **Four, because the palette is four lightnesses of a
            // hue.** Six columns cut across the families: the first row
            // read grey, grey, grey, grey, red, red, which is a grid of
            // twenty-four colours and not a palette.
            COLOR => 4,
            DATE => 7,
            TIME => 6,
            _ => 1,
        }
    }

    /// Cells in the grid, blanks included: a month is always six weeks
    /// so the popup does not change height as you page through it.
    pub fn cells(&self) -> usize {
        match KIND {
            COLOR => SWATCHES.len(),
            DATE => 42,
            // Twenty-four hours, then twelve five-minute marks.
            TIME => 36,
            // A window of the list: visible cell `k` is file `k + first`.
            _ => self.file_count.min(FILE_ROWS),
        }
    }

    pub fn rows(&self) -> usize {
        self.cells().div_ceil(self.cols().max(1))
    }

    /// A cell's width. Not a constant: a file name needs more room than
    /// a swatch, and a popup as wide as its widest kind would make the
    /// colour grid four times too wide.
    pub fn cell_w(&self) -> f32 {
        match KIND {
            COLOR => 32.0,
            DATE => 30.0,
            TIME => 34.0,
            _ => 168.0,
        }
    }

    /// The header above the grid — a month's `‹ name year ›`, and
    /// nothing for the other three.
    pub fn head_h(&self) -> f32 {
        if KIND == DATE {
            HEAD_H + DOW_H
        } else {
            0.0
        }
    }

    /// The day in cell `k`, or `None` for a lead-in or trailing blank.
    pub fn day_at(&self, k: usize) -> Option<u8> {
        let (year, month) = self.shown_month();
        let lead = weekday_of(year, month, 1) as usize;
        let dim = days_in_month(year, month) as usize;
        if k < lead || k >= lead + dim {
            return None;
        }
        Some((k - lead + 1) as u8)
    }

    /// How many names a `FilePicker` holds — not how many it shows, which
    /// is [`Pad::cells`], at most [`FILE_ROWS`] (M57/B19).
    pub fn file_count(&self) -> usize {
        self.file_count
    }

    /// The name in cell `k`.
    pub fn file_at(&self, k: usize) -> &'static str {
        if k < self.file_count {
            self.files[k]
        } else {
            ""
        }
    }

    /// Whether cell `k` is the current selection.
    pub fn is_selected(&self, k: usize) -> bool {
        match KIND {
            COLOR => k == self.color,
            DATE => self.shown_month() == (self.year, self.month) && self.day_at(k) == Some(self.day),
            TIME if k < 24 => k as u8 == self.hour,
            TIME => (k - 24) as u8 * 5 == self.minute,
            _ => k + self.first == self.file,
        }
    }

    /// Take the choice, if one was made. Cleared by the read, so a
    /// second frame does not re-apply it.
    pub fn take_picked(&mut self) -> bool {
        core::mem::replace(&mut self.picked, false)
    }

    /// Set the names a `FilePicker` lists. Truncates at [`MAX_FILES`]
    /// rather than refusing: a picker that showed nothing because the
    /// directory was large is worse than one that shows the first
    /// sixty-four.
    pub fn set_files(&mut self, names: &[&'static str]) {
        self.file_count = names.len().min(MAX_FILES);
        for (i, n) in names.iter().take(MAX_FILES).enumerate() {
            self.files[i] = n;
        }
        self.file = self.file.min(self.file_count.saturating_sub(1));
    }

    /// The month the grid shows: the view, or the chosen month.
    pub fn shown_month(&self) -> (i32, u8) {
        self.view.unwrap_or((self.year, self.month))
    }

    /// Page the **shown** month by `d` — M57/B17. The chosen date does not
    /// move; only a day press commits one.
    pub fn shift_month(&mut self, d: i32) {
        let (y, m) = self.shown_month();
        let total = y * 12 + (m as i32 - 1) + d;
        self.view = Some((total.div_euclid(12), (total.rem_euclid(12) + 1) as u8));
    }

    /// The cell the keyboard or the pointer has lit, if any.
    pub fn hot(&self) -> Option<usize> {
        self.hot
    }

    /// The keyboard's step through the grid — M44/T44.10.
    ///
    /// Left and Right move one cell, Up and Down one row, Home and End to
    /// the ends; the first key starts from the current selection. Enter
    /// and Space choose the lit cell, exactly as releasing over it does.
    /// Returns whether it did anything.
    pub fn key(&mut self, key: Key) -> bool {
        let n = self.cells();
        if n == 0 {
            return false;
        }
        if matches!(key, Key::Enter | Key::Space) {
            return self.hot.is_some_and(|k| self.choose(k));
        }
        let cols = self.cols().max(1);
        let from = self.hot.unwrap_or_else(|| (0..n).find(|&k| self.is_selected(k)).unwrap_or(0));
        let to = match key {
            Key::Right => (from + 1).min(n - 1),
            Key::Left => from.saturating_sub(1),
            Key::Down if from + cols < n => from + cols,
            Key::Down => from,
            Key::Up => from.checked_sub(cols).unwrap_or(from),
            Key::Home => 0,
            Key::End => n - 1,
            _ => return false,
        };
        // The first key lights the selection itself, so the user sees
        // where they are before anything moves.
        let to = if self.hot.is_none() { from } else { to };
        let moved = self.hot != Some(to);
        self.hot = Some(to);
        moved
    }

    /// Which cell a local point is in, or `None` outside the grid.
    ///
    /// Returns `None` for the header too: a press on `‹` is *not* a
    /// cell, and an arithmetic that folded it into row 0 would pick a
    /// day when you paged the month.
    fn cell_at(&self, x: f32, y: f32, pad: Edges) -> Option<usize> {
        let (gx, gy) = (x - pad.left, y - pad.top - self.head_h());
        if gx < 0.0 || gy < 0.0 {
            return None;
        }
        let (c, r) = ((gx / self.cell_w()) as usize, (gy / CELL_H) as usize);
        if c >= self.cols() || r >= self.rows() {
            return None;
        }
        let k = r * self.cols() + c;
        (k < self.cells()).then_some(k)
    }

    /// Take cell `k` as the selection. Answers whether the press was
    /// **taken**, not whether the value changed: re-picking the colour
    /// you already had still closes the popup, and a widget that
    /// declined it would let the press fall through to whatever is
    /// behind the overlay. A blank in a month grid is the one cell that
    /// takes nothing.
    fn choose(&mut self, k: usize) -> bool {
        match KIND {
            COLOR => {
                self.color = k;
                self.picked = true;
                true
            }
            DATE => match self.day_at(k) {
                Some(d) => {
                    // The day, *in the month on show*: this is the commit.
                    (self.year, self.month) = self.shown_month();
                    self.day = d;
                    self.view = None;
                    self.picked = true;
                    true
                }
                None => false,
            },
            // **An hour does not close the clock** — M57/B18. Picking the
            // hour closed the popup, so a time was always HH:00 unless the
            // user opened it twice. The hour is set and the popup stays;
            // the minute is the commit.
            TIME => {
                if k < 24 {
                    self.hour = k as u8;
                } else {
                    self.minute = (k - 24) as u8 * 5;
                    self.picked = true;
                }
                true
            }
            _ => {
                let f = k + self.first;
                if f >= self.file_count {
                    return false;
                }
                self.file = f;
                self.picked = true;
                true
            }
        }
    }
}

/// Month names, for the header. ASCII and short: the system font has no
/// chevron (`examples/glasswall` paid for that lesson) and a header that
/// wrapped would change the popup's height as you paged it.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

impl<const KIND: u8> Widget for Pad<KIND> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    // **Not focusable**: the trigger keeps focus and walks the pad with
    // the arrows (M44/T44.10). M57/A12 had made the pad focusable so focus
    // could move into it, but gave it no keys of its own — and with
    // M44/T44.9's "an overlay that opens takes focus" the first arrow
    // then landed on a pad that declined it and moved focus elsewhere.
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = match KIND {
        COLOR => "colorpad",
        DATE => "datepad",
        TIME => "timepad",
        _ => "filepad",
    };
    const SEEDS: &'static [Seed] = &[Seed::new(<Pad<KIND> as Widget>::KIND, <Pad<KIND> as Widget>::type_defaults, Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::PADDING,
            Fields::BACKGROUND,
            Fields::COLOR,
            Fields::FONT_SIZE,
            Fields::RADIUS,
            Fields::VISIBLE,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text).background(lumen_static::style::Role::Raised))];

    fn type_defaults() -> Style {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            padding: Edges::all(8.0),
            background: Color(0xff, 0xff, 0xff, 0xff),
            color: Color(0x1f, 0x29, 0x37, 0xff),
            font_size: 13.0,
            radius: 10.0,
            shadow: 12.0,
            ..Style::default()
        }
    }
}

impl<const KIND: u8> Node for Pad<KIND> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        if KIND == DATE || KIND == TIME {
            let mut m = |t: &str| cx.text.measure_weighted(t, me.font_size, me.font_weight, f32::INFINITY).w;
            self.digit_w = (m("8"), m("28"));
        }
        let content = Size {
            w: self.cols() as f32 * self.cell_w() + me.padding.horizontal(),
            h: self.head_h() + self.rows() as f32 * CELL_H + me.padding.vertical(),
        };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        self.size = s;
        self.pad = me.padding;
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    /// What a screen reader and the agent are told — M57/A17.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        // The pad is a grid of choices: swatches, days, times, files.
        node.set_role(lumen_static::observe::accesskit::Role::Grid);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        if s.shadow > 0.0 {
            out.shadow(r, s.radius, s.shadow);
        }
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        let (x0, y0) = (r[0] + s.pad.0, r[1] + s.pad.1);
        if KIND == DATE {
            let (year, month) = self.shown_month();
            let m = MONTHS[(month.clamp(1, 12) - 1) as usize];
            out.text_weighted(
                [x0, y0 + 4.0, x0 + 14.0, y0 + HEAD_H],
                "<",
                s.font_size,
                700.0,
                s.fg,
            );
            let title = alloc::format!("{m} {year}");
            out.text_weighted(
                [x0 + 20.0, y0 + 4.0, r[2] - s.pad.2 - 20.0, y0 + HEAD_H],
                &title,
                s.font_size,
                700.0,
                s.fg,
            );
            out.text_weighted(
                [r[2] - s.pad.2 - 14.0, y0 + 4.0, r[2] - s.pad.2, y0 + HEAD_H],
                ">",
                s.font_size,
                700.0,
                s.fg,
            );
            // The weekday strip, one label per column, dimmed: it is a
            // heading, not a row of cells, and nothing in it is
            // pressable. **Dimmed to 0xb8, not 0x99**: at 0x99 the strip
            // measured 4.04:1 on the pad's white (M54/T54.6,
            // `docs/results-a11y-platform.md`) and 4.30:1 on the showcase's
            // pad (M57/A8's demo contrast gate), both under AA. Both
            // programs raised it; this is the higher of the two.
            let dim = Color(s.fg.0, s.fg.1, s.fg.2, 0xb8);
            for (c, d) in DOW.iter().enumerate() {
                let x = x0 + c as f32 * self.cell_w();
                out.text_weighted(
                    [x + 6.0, y0 + HEAD_H + 2.0, x + self.cell_w(), y0 + HEAD_H + DOW_H],
                    d,
                    s.font_size - 2.0,
                    600.0,
                    dim,
                );
            }
        }
        let gy = y0 + self.head_h();
        // **The seam between the hours and the minutes.** Without it a
        // clock reads as one grid of thirty-six numbers, which is what
        // the first render looked like: `18 19 20 21 22 23` sitting
        // directly above `:00 :05 :10`, with nothing saying the two rows
        // mean different things.
        if KIND == TIME {
            let y = gy + 4.0 * CELL_H;
            out.rect([x0, y - 1.0, x0 + self.cols() as f32 * self.cell_w(), y],
                     Color(s.fg.0, s.fg.1, s.fg.2, 0x33));
        }
        for k in 0..self.cells() {
            let (c, row) = (k % self.cols(), k / self.cols());
            let x = x0 + c as f32 * self.cell_w();
            let y = gy + row as f32 * CELL_H;
            let cell = [x, y, x + self.cell_w(), y + CELL_H];
            let selected = self.is_selected(k);
            if KIND == COLOR {
                // **A ring, a gap, then the swatch** — M57/B16. The ring
                // was the text colour straight against the swatch, so on a
                // dark swatch the selection all but vanished. The gap in
                // the pad's own ground separates them whatever the swatch.
                let sw = [cell[0] + 5.0, cell[1] + 3.0, cell[2] - 5.0, cell[3] - 3.0];
                if selected {
                    out.rounded_rect([sw[0] - 3.0, sw[1] - 3.0, sw[2] + 3.0, sw[3] + 3.0], s.fg, 8.0);
                    out.rounded_rect([sw[0] - 1.5, sw[1] - 1.5, sw[2] + 1.5, sw[3] + 1.5], s.bg, 6.5);
                }
                out.rounded_rect(sw, SWATCHES[k.min(SWATCHES.len() - 1)], 5.0);
                continue;
            }
            let text = match KIND {
                DATE => match self.day_at(k) {
                    Some(d) => alloc::format!("{d}"),
                    None => continue,
                },
                TIME if k < 24 => alloc::format!("{k}"),
                TIME => alloc::format!(":{:02}", (k - 24) * 5),
                _ => String::from(self.file_at(k + self.first)),
            };
            if selected {
                out.rounded_rect(cell, s.fg, 6.0);
            } else if self.hot == Some(k) {
                out.rounded_rect(cell, Color(s.fg.0, s.fg.1, s.fg.2, 0x22), 6.0);
            }
            let col = if selected { s.bg } else { s.fg };
            // A number is centred in its cell; a file name or a minute
            // label stays left-aligned, where a list reads from.
            let tx = match (KIND, text.len()) {
                (DATE, 1) | (TIME, 1) => cell[0] + ((cell[2] - cell[0]) - self.digit_w.0) / 2.0,
                (DATE, 2) | (TIME, 2) => cell[0] + ((cell[2] - cell[0]) - self.digit_w.1) / 2.0,
                _ => cell[0] + 4.0,
            };
            out.text_weighted(
                [tx, y + CELL_H / 2.0 - s.font_size * 0.6, cell[2] - 4.0, cell[3]],
                &text,
                s.font_size,
                s.font_weight,
                col,
            );
        }
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        let pad = self.pad;
        match ev {
            // The pointer left: no cell is hot (M57/B50).
            Event::Leave => self.hot.take().is_some(),
            // A long file list scrolls by rows (M57/B19). Declined at an
            // end, so the wheel chains to what is behind, as a scroller's
            // does (C4).
            Event::Scroll { dy, .. } if KIND == FILE => {
                let rows = (lumen_static::num::round(dy.abs() / CELL_H) as usize).max(1);
                let last = self.file_count.saturating_sub(FILE_ROWS);
                let to = if *dy < 0.0 { (self.first + rows).min(last) } else { self.first.saturating_sub(rows) };
                core::mem::replace(&mut self.first, to) != to
            }
            Event::PointerMove { .. } => {
                let was = self.hot;
                self.hot = self.cell_at(cx.local.x, cx.local.y, pad);
                was != self.hot
            }
            // **On the release**, the way every menu commits — and for
            // the same reason `OptionList` does (T27.9): a press that
            // drags off the cell must not choose it.
            Event::PointerUp { .. } => {
                if KIND == DATE && cx.local.y - pad.top < HEAD_H {
                    // **Only the arrows page** — M57/B17. Any press in the
                    // header paged, split at the middle, so the right half
                    // of "Oct 2026" went forward and the left half back.
                    // The arrows are drawn 14 px wide at each end; their
                    // targets are 28 px, and the title between does nothing.
                    let x = cx.local.x;
                    if x < pad.left + ARROW_HIT {
                        self.shift_month(-1);
                    } else if x > self.size.w - pad.right - ARROW_HIT {
                        self.shift_month(1);
                    }
                    return true;
                }
                match self.cell_at(cx.local.x, cx.local.y, pad) {
                    Some(k) => self.choose(k),
                    None => false,
                }
            }
            _ => false,
        }
    }

    lumen_static::leaf_common!(Pad<KIND>, 0);
}

// ---- the picker ------------------------------------------------------

pub const PART_TRIGGER: &str = "picker-trigger";
pub const PART_POPUP: &str = "picker-popup";
pub const PART_PAD: &str = "picker-pad";

/// Table slots, so `MAX_PICKER_NODES` is one number rather than a
/// const-eval expression repeated four times.
pub const MAX_PICKER_NODES: usize = 8;

/// A trigger that shows the value, and a popup of cells that changes it.
pub struct Picker<const KIND: u8 = COLOR> {
    /// The cells. Public because the application seeds a file list and
    /// reads a date out of it.
    pub pad: Popup<Pad<KIND>>,
    /// The trigger's laid-out box, so a press can be told from a press
    /// somewhere else.
    trigger: Size,
    /// What a screen reader announces this control as — M54/T54.3
    /// (R15-F3). Empty by default, and an empty one is not published: a
    /// control with no visible text of its own needs its application to
    /// say what it is for. A field rather than a relation to a sibling
    /// `Label`, so it works inside a `Repeat` row.
    ///
    /// **Only with `a11y`**: a build nobody can read aloud — the firmware
    /// — does not carry it (its `Ui` is a const-asserted size). Write it
    /// through `set_label`, which every build has.
    #[cfg(feature = "a11y")]
    pub label: &'static str,
    /// The trigger text's line height, from layout.
    line_h: f32,
}

impl<const KIND: u8> Default for Picker<KIND> {
    fn default() -> Self {
        Picker { pad: Popup::new(Pad::<KIND>::default()), trigger: Size { w: 0.0, h: 0.0 }, line_h: 0.0,
            #[cfg(feature = "a11y")]
            label: "",
        }
    }
}

impl<const KIND: u8> Picker<KIND> {
    const TRIGGER: usize = 1;
    const POPUP: usize = 2;
    /// The space between the trigger and its pad, on whichever side the
    /// pad opens. One constant for both paths, because the flip path
    /// once left it out and a pad opened above ended 4 px inside its own
    /// trigger (M57/D6).
    const GAP: f32 = 4.0;

    pub fn new() -> Self {
        Self::default()
    }

    /// The names a `FilePicker` lists.
    pub fn with_files(mut self, names: &[&'static str]) -> Self {
        self.pad.content.set_files(names);
        self
    }

    /// The cells, to seed or to read.
    ///
    /// The popup's content is `pub` on `Popup`; this is the shorter path
    /// and the one an application uses — `picker.pad.content` reads as
    /// two pieces of plumbing and one widget.
    pub fn cells(&self) -> &Pad<KIND> {
        &self.pad.content
    }

    pub fn cells_mut(&mut self) -> &mut Pad<KIND> {
        &mut self.pad.content
    }

    /// Show the pad, without a press.
    ///
    /// For a demo or a test that wants the open state on screen —
    /// `Chooser::open` exists for the same reason (T27.9). Not a
    /// toggle: "open it" and "flip it" are different requests and a
    /// caller that wanted the first and got the second closed the thing
    /// it meant to show.
    pub fn open(&mut self) {
        self.pad.open = true;
    }

    pub fn is_open(&self) -> bool {
        self.pad.open
    }

    /// What the trigger reads — and what `ui.resolve` answers with, so a
    /// drive can assert the *value* rather than a pixel.
    pub fn text(&self) -> String {
        let c = &self.pad.content;
        match KIND {
            COLOR => {
                let s = SWATCHES[c.color.min(SWATCHES.len() - 1)];
                alloc::format!("#{:02x}{:02x}{:02x}", s.0, s.1, s.2)
            }
            DATE => alloc::format!("{}-{:02}-{:02}", c.year, c.month, c.day),
            TIME => alloc::format!("{:02}:{:02}", c.hour, c.minute),
            _ => String::from(c.file_at(c.file)),
        }
    }
}

pub struct PickerTable<const KIND: u8>;

impl<const KIND: u8> PickerTable<KIND> {
    const N: usize = 1 + 1 + <Popup<Pad<KIND>> as Widget>::NODE_COUNT;

    const ARRAY: [NodeDesc; MAX_PICKER_NODES] = {
        assert!(Self::N <= MAX_PICKER_NODES, "Picker exceeds MAX_PICKER_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_PICKER_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        t[1] = NodeDesc::new(0, 0, 2, 0, NodeDesc::FOCUSABLE).with_id(PART_TRIGGER);
        lumen_static::widget::splice_child(&mut t, 2, <Popup<Pad<KIND>> as Widget>::SUB_TABLE, 0);
        t[2] = t[2].with_id(PART_POPUP);
        t[3] = t[3].with_id(PART_PAD);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_PICKER_NODES] = {
        let mut s = [Seed::WIDGET; MAX_PICKER_NODES];
        s[0] = Seed::new(<Picker<KIND> as Widget>::KIND, <Picker<KIND> as Widget>::type_defaults, Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::VISIBLE]));
        // The trigger is painted by this widget, not by a child type —
        // so it seeds from the same defaults and reads the fields a
        // stylesheet can set on it.
        s[1] = Seed::new("picker-trigger", Picker::<KIND>::trigger_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::RADIUS,
                Fields::BORDER,
                Fields::VISIBLE,
                Fields::CURSOR,
                Fields::HOVER_DIM,
                Fields::PRESSED_DIM,
            ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text).background(lumen_static::style::Role::Raised).border(lumen_static::style::Role::Border));
        lumen_static::widget::splice_seeds(&mut s, 2, <Popup<Pad<KIND>> as Widget>::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<const KIND: u8> Widget for Picker<KIND> {
    const NODE_COUNT: usize = PickerTable::<KIND>::N;
    const DEPTH: usize = 1 + <Popup<Pad<KIND>> as Widget>::DEPTH;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = PickerTable::<KIND>::SLICE;
    const KIND: &'static str = match KIND {
        COLOR => "colorpicker",
        DATE => "datepicker",
        TIME => "timepicker",
        _ => "filepicker",
    };
    const SEEDS: &'static [Seed] = PickerTable::<KIND>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style { width: Length::Shrink, height: Length::Shrink, ..Style::default() }
    }
}

impl<const KIND: u8> Picker<KIND> {
    /// The trigger's own defaults: a bordered field, which is what
    /// T27.1's stroked rect was built for and what makes these four
    /// read as *inputs* rather than as buttons.
    pub fn trigger_defaults() -> Style {
        Style {
            width: Length::Fixed(160.0),
            height: Length::Fixed(34.0),
            padding: Edges::all(8.0),
            background: Color(0xff, 0xff, 0xff, 0xff),
            color: Color(0x1f, 0x29, 0x37, 0xff),
            border_width: 1,
            border_color: Color(0xcb, 0xd2, 0xdb, 0xff),
            font_size: 14.0,
            radius: 8.0,
            // A control that opens something says so (M57/B16).
            cursor: lumen_static::style::Cursor::Pointer,
            hover_dim: 0.04,
            pressed_dim: 0.10,
            ..Style::default()
        }
    }
}

impl<const KIND: u8> Node for Picker<KIND> {
    lumen_static::node_count!();
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    /// Arrows walk the grid (M44/T44.10), so they never move focus off
    /// the trigger — which is this node: `node(1)` answers `self`.
    fn owns_arrows(&self) -> bool {
        true
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        // **The pad's choice is taken here**, where `&mut self` and the
        // state array are both in hand — `on_event` reaches only the
        // node that was hit, and the node hit inside an open popup is
        // the *pad*. `Chooser` takes `OptionList`'s the same way and for
        // the same reason (T27.9).
        if self.pad.content.take_picked() {
            self.pad.open = false;
        }
        // A closed pad forgets its lit cell, so the keyboard starts from
        // the selection when it opens again (M44/T44.10).
        if !self.pad.open {
            self.pad.content.hot = None;
        }
        let t = lumen_static::ops::resolve_self(
            limits,
            &style[i + Self::TRIGGER],
            Size { w: 0.0, h: 0.0 },
        );
        lumen_static::ops::record_painted(
            st,
            i + Self::TRIGGER,
            limits,
            t,
            &style[i + Self::TRIGGER],
        );
        self.trigger = t;
        self.line_h = cx.text.line_height(style[i + Self::TRIGGER].font_size);
        // Flipping above clears the trigger (M57/A18).
        // The anchor is a gap under the trigger, so flipping crosses that
        // gap, the trigger, and a gap above it — as `Tooltip` does.
        self.pad.above_by = t.h + 2.0 * Self::GAP;
        self.pad.layout(cx, limits, style, st, i + Self::POPUP);
        let s = lumen_static::ops::resolve_self(limits, &me, t);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        st[i + Self::TRIGGER].pos = origin;
        self.pad.place(
            Point { x: origin.x, y: origin.y + st[i + Self::TRIGGER].size.h + Self::GAP },
            style,
            st,
            i + Self::POPUP,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i + Self::TRIGGER];
        let r = lumen_static::ops::rect_of(s);
        if r[2] > r[0] && r[3] > r[1] {
            out.rounded_rect(r, s.bg, s.radius);
            lumen_static::ops::border(st, i + Self::TRIGGER, out);
            let mut x = r[0] + s.pad.0;
            if KIND == COLOR {
                // The swatch *is* the value; the hex is the label.
                let c = SWATCHES[self.pad.content.color.min(SWATCHES.len() - 1)];
                let mid = (r[1] + r[3]) / 2.0;
                out.rounded_rect([x, mid - 8.0, x + 16.0, mid + 8.0], c, 4.0);
                x += 24.0;
            }
            // Centred on the line box, as a button centres its label —
            // the renderer puts the capitals in its middle (M57/B19: it
            // was placed from a 0.6 em guess and sat high).
            let text = self.text();
            let y = (r[1] + r[3] - self.line_h) / 2.0;
            let chev = 16.0;
            out.text_weighted([x, y, r[2] - s.pad.2 - chev, y + self.line_h], &text, s.font_size, s.font_weight, s.fg);
            // A chevron: this opens (M57/B16).
            let (cx, cy, d) = (r[2] - s.pad.2 - chev / 2.0, (r[1] + r[3]) / 2.0, 3.5);
            out.line(Point { x: cx - d, y: cy - d / 2.0 }, Point { x: cx, y: cy + d / 2.0 }, 1.5, s.fg);
            out.line(Point { x: cx, y: cy + d / 2.0 }, Point { x: cx + d, y: cy - d / 2.0 }, 1.5, s.fg);
        }
        self.pad.paint(st, i + Self::POPUP, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        // The open pad first: it is an overlay, and it is above the
        // trigger it hangs from.
        if let Some(h) = self.pad.hit(st, i + Self::POPUP, x, y) {
            return Some(h);
        }
        if lumen_static::hit::contains(&st[i + Self::TRIGGER], x, y) {
            let s = &st[i + Self::TRIGGER];
            return Some(lumen_static::Hit {
                node: (i + Self::TRIGGER) as u32,
                inner: None,
                local: Point { x: x - s.pos.x, y: y - s.pos.y },
            });
        }
        None
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // A screen reader's open and close (M54/T54.4).
            Event::Action { action: lumen_static::Action::Expand, .. } => {
                let was = self.pad.open;
                self.pad.open = true;
                !was
            }
            Event::Action { action: lumen_static::Action::Collapse, .. } => {
                let was = self.pad.open;
                self.pad.open = false;
                was
            }
            Event::Key { key: Key::Escape, down: true } if self.pad.open => {
                self.pad.open = false;
                true
            }
            // **The keyboard walks the open grid** (M44/T44.10). The pad
            // is drawn, not focused; the trigger — this node — keeps
            // focus, so its keys go to the pad: arrows light a cell, and
            // Enter or Space chooses the lit one. With nothing lit, Enter
            // still opens and closes, as a click does.
            Event::Key {
                key: k @ (Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End),
                down: true,
            } if self.pad.open => self.pad.content.key(*k),
            Event::Key { key: k @ (Key::Enter | Key::Space), down: true }
                if self.pad.open && self.pad.content.hot().is_some() =>
            {
                self.pad.content.key(*k)
            }
            // Down on a closed picker opens it, as every dropdown does.
            Event::Key { key: Key::Down, down: true } => {
                self.pad.open = true;
                true
            }
            // **Only the trigger toggles** — M57/B16. A press on the pad's
            // own padding or its weekday strip is declined by the pad and
            // bubbled here, and this flipped `open` for any release: the
            // popup closed under a press inside it. The trigger is this
            // node's own index (`node(1)` is `self`), so a release aimed
            // at it arrives with `node == target`; one bubbled up from the
            // pad does not.
            Event::PointerUp { .. } | Event::Key { key: Key::Enter | Key::Space, down: true }
                if cx.target != cx.node =>
            {
                false
            }
            // **A press on the trigger opens it; a press anywhere else
            // does not reach here.** `dispatch_overlaid` consumes a
            // press that misses every open overlay — so this arm cannot
            // re-open what that press just closed, which is the
            // second-press-closes behaviour a dropdown needs and the one
            // `Chooser` had to reach for focus to get (T27.9).
            Event::PointerUp { .. } | Event::Key { key: Key::Enter | Key::Space, down: true } => {
                self.pad.open = !self.pad.open;
                true
            }
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            0 | 1 => self,
            _ => self.pad.node(k - Self::POPUP),
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            0 | 1 => self,
            _ => self.pad.node_mut(k - Self::POPUP),
        }
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::ComboBox);
        if !self.label.is_empty() {
            node.set_label(self.label);
        }
        node.set_value(self.text());
        node.set_expanded(self.pad.open);
        node.add_action(if self.pad.open { accesskit::Action::Collapse } else { accesskit::Action::Expand });
    }
}

impl<const KIND: u8, A> lumen_static::event::Handlers<A> for Picker<KIND> {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}

/// A pad carries no handler of its own, like the picker around it. It had
/// no `Handlers` impl at all, so a `Pad` — which this crate exports —
/// could not be placed in an application's tree: found when the role
/// table's catalogue became one (M54/T54.4).
impl<const KIND: u8, A> lumen_static::event::Handlers<A> for Pad<KIND> {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}

/// A grid of swatches.
pub type ColorPicker = Picker<COLOR>;
/// A month, with `<` and `>`.
pub type DatePicker = Picker<DATE>;
/// Hours over five-minute marks.
///
/// **One grid, not two lists.** A clock is an hour list and a minute list;
/// putting both in one popup of cells — twenty-four hours, then twelve
/// five-minute marks — makes it one hit test instead of two. The seam
/// painted between them is what keeps it reading as two.
pub type TimePicker = Picker<TIME>;
/// A list of names the application supplies.
///
/// **It owns no data.** It does not read a directory; the names are handed
/// to it (`Picker::with_files`), the way a `DataGrid`'s rows are, because
/// a file list belongs to whatever read the directory.
pub type FilePicker = Picker<FILE>;

impl<const KIND: u8> Picker<KIND> {
    /// Name this control for a screen reader (M54/T54.3). A no-op in a build without `a11y`, which
    /// carries no name.
    #[allow(unused_variables)]
    pub fn set_label(&mut self, label: &'static str) {
        #[cfg(feature = "a11y")]
        {
            self.label = label;
        }
    }
}
