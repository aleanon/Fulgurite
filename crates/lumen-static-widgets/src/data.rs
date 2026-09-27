//! M27/T27.10 — the data group: `Pagination`, `Tree`, `DataGrid`.
//!
//! # Where the data lives, and why the three differ
//!
//! A static tree makes this an explicit choice rather than a default,
//! and the three answer it differently *because their data differs*:
//!
//! * `Pagination` has none — a page count and a current page.
//! * `Tree` owns a fixed array of `(label, depth)`. A tree's shape is
//!   part of its state: collapsing a node hides its children, and that
//!   is arithmetic over the array rather than anything the application
//!   should have to re-derive.
//! * `DataGrid` owns **nothing**. Its rows are a `Repeat` the
//!   application binds, the way `Select`'s are, because a grid is a view
//!   of a table someone else holds and copying it in would be a second
//!   copy to keep in step. It is the one of the three where `Repeat` is
//!   the right answer, and `docs/plan-widget-parity.md` T27.9 records
//!   why the choosers are not.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

// ---- Pagination ------------------------------------------------------

/// The most page buttons shown at once.
pub const MAX_PAGES: usize = 9;

/// A page number rendered into a stack buffer.
///
/// **Not a table of literals.** It was a thirty-three entry table
/// indexed with `.min(len - 1)`, which labelled every page past its end
/// "32" — silently, because the showcase seeds thirty pages and nothing
/// ever asked for a thirty-fourth (M27/T27.11's review, finding 15). A
/// pagination whose data outgrows its labels is the normal case, not
/// the exotic one.
///
/// Five digits: 65 535 pages is `u16::MAX`, and a paginator with more
/// than that has a different problem.
pub struct PageLabel([u8; 5], usize);

impl PageLabel {
    pub fn new(mut n: usize) -> PageLabel {
        let mut buf = [b'0'; 5];
        let mut i = buf.len();
        if n == 0 {
            i -= 1;
        }
        while n > 0 && i > 0 {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
        PageLabel(buf, i)
    }

    pub fn as_str(&self) -> &str {
        // Every byte written is an ASCII digit and `i` never passes the
        // end, so this cannot fail; `unwrap_or` rather than `unwrap`
        // because the core carries no panic path it can avoid.
        core::str::from_utf8(&self.0[self.1..]).unwrap_or("?")
    }
}

/// Page numbers, with a previous and a next.
///
/// # The window — M57/B49
///
/// Up to [`MAX_PAGES`] pages are all shown. Past that the strip is nine
/// slots: **the first page, a gap, five pages, a gap, the last page** —
/// a gap is `…`, or the page it stands for when it hides only one. The
/// first and last are always one press away.
///
/// **A press never moves the numbers.** The strip was re-centred on
/// every change, so pressing "5" shifted the numbers and put another one
/// under the pointer — what this type's own rustdoc said it did not do.
/// Now only the keyboard and the ‹ › arrows slide the five middle pages,
/// and only when the page reaches their edge; a press on a number goes
/// there and leaves the strip still. A press on a gap jumps past it.
pub struct Pagination {
    pages: usize,
    page: usize,
    /// The first of the five middle pages, when windowed.
    start: usize,
    /// The slot under the pointer, for the hover tint.
    hot: Option<usize>,
    /// Laid-out width, so `on_event` can turn `local.x` into a button.
    width: f32,
    /// Each visible number's measured width, so `paint` can centre it.
    ///
    /// **Measured at layout, centred at paint** — `Strip` does the same,
    /// and for the same reason: `Painter` cannot measure. Before this
    /// the digit was drawn left-aligned in the *slot* while its
    /// selection pill was centred in it, so the glyph landed 3 px
    /// outside the pill and was painted in the pill's contrast colour on
    /// the control's own background: **the current page read as an empty
    /// lozenge** (M27/T27.11's review, finding 1).
    widths: [f32; MAX_PAGES],
}

impl Default for Pagination {
    fn default() -> Self {
        Pagination { pages: 1, page: 0, start: 2, hot: None, width: 0.0, widths: [0.0; MAX_PAGES] }
    }
}

/// What one slot of the page strip shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageSlot {
    Page(usize),
    /// `…`, standing for the pages between its neighbours.
    Gap,
}

/// The middle pages a windowed strip shows.
const MIDDLE: usize = 5;
/// A gap's label.
const GAP: &str = "…";

impl Pagination {
    pub fn new(pages: usize) -> Self {
        Pagination { pages: pages.max(1), page: 0, start: 2, hot: None, width: 0.0, widths: [0.0; MAX_PAGES] }
    }

    pub fn pages(&self) -> usize {
        self.pages
    }

    pub fn page(&self) -> usize {
        self.page
    }

    /// Go to `p`, clamped, and slide the middle pages if `p` is at or
    /// past their edge — the keyboard's and the arrows' move. Returns
    /// whether the page changed.
    pub fn go(&mut self, p: usize) -> bool {
        let moved = self.set_page(p);
        self.reveal();
        moved
    }

    /// Go to `p` and leave the strip where it is — a press on a number.
    fn set_page(&mut self, p: usize) -> bool {
        let p = p.min(self.pages - 1);
        let moved = p != self.page;
        self.page = p;
        moved
    }

    fn windowed(&self) -> bool {
        self.pages > MAX_PAGES
    }

    /// The largest `start`: the five end at the second-to-last page.
    fn max_start(&self) -> usize {
        self.pages.saturating_sub(2 + MIDDLE)
    }

    /// Slide the middle pages so the page is inside them and off their
    /// edge, if it is not already.
    fn reveal(&mut self) {
        if !self.windowed() {
            return;
        }
        let (s, e) = (self.start, self.start + MIDDLE - 1);
        let p = self.page;
        if p <= s || p >= e {
            self.start = p.saturating_sub(MIDDLE / 2).clamp(2, self.max_start());
        }
    }

    pub fn next_page(&mut self) -> bool {
        self.go(self.page + 1)
    }

    pub fn prev_page(&mut self) -> bool {
        self.go(self.page.saturating_sub(1))
    }

    /// How many number slots the strip has.
    pub fn count(&self) -> usize {
        self.pages.min(MAX_PAGES)
    }

    /// What slot `k` (0-based, arrows excluded) shows.
    pub fn slot(&self, k: usize) -> PageSlot {
        if !self.windowed() {
            return PageSlot::Page(k);
        }
        let s = self.start;
        match k {
            0 => PageSlot::Page(0),
            1 if s == 2 => PageSlot::Page(1),
            1 => PageSlot::Gap,
            7 if s + MIDDLE == self.pages - 2 => PageSlot::Page(self.pages - 2),
            7 => PageSlot::Gap,
            8 => PageSlot::Page(self.pages - 1),
            _ => PageSlot::Page(s + k - 2),
        }
    }

    /// Slots across the control: `<`, the numbers, `>`.
    fn slots(&self) -> usize {
        self.count() + 2
    }

    fn slot_at(&self, x: f32) -> Option<usize> {
        if self.width <= 0.0 {
            return None;
        }
        let each = self.width / self.slots() as f32;
        if each <= 0.0 || x < 0.0 {
            return None;
        }
        Some(((x / each) as usize).min(self.slots() - 1))
    }
}

impl Widget for Pagination {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = "pagination";
    const SEEDS: &'static [Seed] =
        &[Seed::new(<Pagination as Widget>::KIND, <Pagination as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::RADIUS,
                Fields::FOCUS_RING,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]),).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Accent).background(lumen_static::style::Role::Raised))];

    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(320.0),
            height: Length::Fixed(36.0),
            background: Color(0xf3, 0xf5, 0xf9, 0xff),
            color: Color(0x2f, 0x6b, 0xff, 0xff),
            font_size: 14.0,
            radius: 8.0,
            cursor: lumen_static::style::Cursor::Pointer,
            ..Style::default()
        }
    }
}

impl Node for Pagination {
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
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: 36.0 },
        );
        self.width = s.w;
        for k in 0..self.count() {
            let (text, current) = match self.slot(k) {
                PageSlot::Page(p) => (PageLabel::new(p + 1), p == self.page),
                PageSlot::Gap => (PageLabel::new(0), false),
            };
            let t = if self.slot(k) == PageSlot::Gap { GAP } else { text.as_str() };
            self.widths[k] = cx
                .text
                .measure_weighted(t, me.font_size, if current { 600.0 } else { me.font_weight }, limits.max_w)
                .w;
        }
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        let each = w / self.slots() as f32;
        // `<`, the numbers, `>`. Arrows are two lines each, so no glyph
        // and no font — `Icon`'s argument, one widget over.
        let arrow = |out: &mut dyn Painter, cx0: f32, back: bool, col: Color| {
            let (mid, k) = (r[1] + h / 2.0, 4.0);
            let tip = if back { cx0 - k / 2.0 } else { cx0 + k / 2.0 };
            let tail = if back { cx0 + k / 2.0 } else { cx0 - k / 2.0 };
            out.line(Point { x: tail, y: mid - k }, Point { x: tip, y: mid }, 2.0, col);
            out.line(Point { x: tip, y: mid }, Point { x: tail, y: mid + k }, 2.0, col);
        };
        let dim = Color(s.fg.0, s.fg.1, s.fg.2, 0x60);
        arrow(out, r[0] + each / 2.0, true, if self.page == 0 { dim } else { s.fg });
        arrow(
            out,
            r[2] - each / 2.0,
            false,
            if self.page + 1 >= self.pages { dim } else { s.fg },
        );
        // Hover (M57/B49): a tint on the slot under the pointer, arrows
        // included, unless it is the current page's pill.
        if let Some(k) = self.hot {
            let current = k >= 1 && k <= self.count() && self.slot(k - 1) == PageSlot::Page(self.page);
            if !current {
                let x = r[0] + each * k as f32;
                out.rounded_rect([x + 3.0, r[1] + 4.0, x + each - 3.0, r[3] - 4.0], Color(s.fg.0, s.fg.1, s.fg.2, 0x1c), 6.0);
            }
        }
        for k in 0..self.count() {
            let x = r[0] + each * (k as f32 + 1.0);
            let (label, current) = match self.slot(k) {
                PageSlot::Page(p) => (PageLabel::new(p + 1), p == self.page),
                PageSlot::Gap => (PageLabel::new(0), false),
            };
            if current {
                out.rounded_rect([x + 3.0, r[1] + 4.0, x + each - 3.0, r[3] - 4.0], s.fg, 6.0);
            }
            let col = if current { s.bg } else { s.fg };
            // Page numbers are 1-based on screen and 0-based in the
            // field: every user-facing pagination counts from one, and
            // every array does not.
            let text = if self.slot(k) == PageSlot::Gap { GAP } else { label.as_str() };
            // Centred in the slot, from the width `layout` measured —
            // the same centre the pill above uses, so the two agree by
            // construction rather than by coincidence.
            let tw = self.widths[k].min(each);
            let tx = x + (each - tw) / 2.0;
            out.text_weighted(
                [tx, r[1] + h / 2.0 - s.font_size * 0.6, tx + tw, r[3]],
                text,
                s.font_size,
                if current { 600.0 } else { s.font_weight },
                col,
            );
        }
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            Event::PointerDown { .. } => match self.slot_at(cx.local.x) {
                Some(0) => {
                    self.prev_page();
                    true
                }
                Some(k) if k == self.slots() - 1 => {
                    self.next_page();
                    true
                }
                // A number: go there and leave the strip still. A gap: jump
                // past it, and let the middle follow.
                Some(k) => {
                    match self.slot(k - 1) {
                        PageSlot::Page(p) => {
                            self.set_page(p);
                        }
                        PageSlot::Gap if k - 1 == 1 => {
                            self.go(self.start - 1);
                        }
                        PageSlot::Gap => {
                            self.go(self.start + MIDDLE);
                        }
                    }
                    true
                }
                None => false,
            },
            Event::Hover { .. } => {
                let k = self.slot_at(cx.local.x);
                core::mem::replace(&mut self.hot, k) != k
            }
            Event::Leave => self.hot.take().is_some(),
            Event::Key { key: Key::Right, down: true } => self.next_page(),
            Event::Key { key: Key::Left, down: true } => self.prev_page(),
            Event::Key { key: Key::Home, down: true } => self.go(0),
            Event::Key { key: Key::End, down: true } => self.go(usize::MAX),
            _ => false,
        }
    }

    lumen_static::leaf_common!(Pagination, 0);

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::ListBox);
        node.set_size_of_set(self.pages);
        node.set_position_in_set(self.page + 1);
        node.set_value(alloc::format!("{} of {}", self.page + 1, self.pages));
    }
}

impl<S> lumen_static::event::Handlers<S> for Pagination {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- Tree ------------------------------------------------------------

/// The most nodes one [`Tree`] holds.
pub const MAX_TREE_NODES: usize = 32;

/// One row of a tree, as the author writes it.
#[derive(Clone, Copy, Default)]
pub struct Branch {
    pub label: &'static str,
    /// Nesting level; 0 is a root.
    pub depth: u8,
}

impl Branch {
    pub const fn new(label: &'static str, depth: u8) -> Self {
        Branch { label, depth }
    }
}

/// A hierarchy, flattened.
///
/// **The shape is the widget's, not the application's.** Collapsing a
/// node hides everything under it, and "under it" is a run of rows with
/// a deeper `depth` — arithmetic over the array, which an application
/// should not have to re-derive every frame. That is the difference
/// between this and [`DataGrid`], whose rows it does not own at all.
pub struct Tree {
    rows: [Branch; MAX_TREE_NODES],
    count: usize,
    /// Whether each row is expanded. A row with no children is a leaf
    /// and this is ignored for it.
    open: [bool; MAX_TREE_NODES],
    /// The row the pointer is over: a subtle tint.
    hot: Option<usize>,
    /// The selected row: the strong fill, and where the keyboard is —
    /// M57/B50. **Controlled** like `Toggle::on`: a press or an arrow
    /// moves it here, and an application that owns the selection writes
    /// it back; [`Tree::take_chosen`] reports a press or Enter once.
    pub selected: Option<usize>,
    chosen: bool,
    height: f32,
    /// The padding `layout` last resolved, so a hit test agrees with
    /// what `paint` drew whatever a stylesheet set.
    pad: Edges,
}

/// A tree row's height.
const TREE_ROW_H: f32 = 26.0;
/// How far each level is indented.
const INDENT: f32 = 18.0;

impl Default for Tree {
    fn default() -> Self {
        Tree {
            rows: [Branch::default(); MAX_TREE_NODES],
            count: 0,
            open: [true; MAX_TREE_NODES],
            hot: None,
            selected: None,
            chosen: false,
            height: 0.0,
            pad: Edges::all(8.0),
        }
    }
}

impl Tree {
    pub fn new(rows: &[Branch]) -> Self {
        let mut t = Tree::default();
        for (i, b) in rows.iter().take(MAX_TREE_NODES).enumerate() {
            t.rows[i] = *b;
            t.count = i + 1;
        }
        t
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn label(&self, i: usize) -> &'static str {
        self.rows[i.min(MAX_TREE_NODES - 1)].label
    }

    pub fn is_open(&self, i: usize) -> bool {
        self.open[i.min(MAX_TREE_NODES - 1)]
    }

    /// Whether row `i` has anything under it — the next row is deeper.
    pub fn has_children(&self, i: usize) -> bool {
        i + 1 < self.count && self.rows[i + 1].depth > self.rows[i].depth
    }

    /// Expand or collapse row `i`. A leaf cannot be toggled, and saying
    /// so is the difference between a chevron that does nothing and one
    /// that is not drawn.
    pub fn toggle(&mut self, i: usize) -> bool {
        if i >= self.count || !self.has_children(i) {
            return false;
        }
        self.open[i] = !self.open[i];
        true
    }

    /// Whether row `i` is visible: every ancestor is expanded.
    ///
    /// Walks *backwards* to the nearest shallower row, which is the
    /// parent — the flattening means a parent is the last row above with
    /// a smaller depth, and no separate index is needed.
    pub fn is_visible(&self, i: usize) -> bool {
        if i >= self.count {
            return false;
        }
        let mut depth = self.rows[i].depth;
        let mut k = i;
        while k > 0 && depth > 0 {
            k -= 1;
            if self.rows[k].depth < depth {
                if !self.open[k] {
                    return false;
                }
                depth = self.rows[k].depth;
            }
        }
        true
    }

    /// The row a press or Enter chose since the last call — and clears it.
    pub fn take_chosen(&mut self) -> Option<usize> {
        if core::mem::take(&mut self.chosen) { self.selected } else { None }
    }

    /// The parent of row `i`: the last row above it with a smaller depth.
    pub fn parent(&self, i: usize) -> Option<usize> {
        let d = self.rows.get(i)?.depth;
        (0..i).rev().find(|&k| self.rows[k].depth < d)
    }

    /// The visible row `step` places after (or before) `i`, if any.
    fn step(&self, i: usize, down: bool) -> Option<usize> {
        if down {
            (i + 1..self.count).find(|&k| self.is_visible(k))
        } else {
            (0..i).rev().find(|&k| self.is_visible(k))
        }
    }

    /// Select `i`. Answers whether the selection moved.
    fn select(&mut self, i: usize) -> bool {
        core::mem::replace(&mut self.selected, Some(i)) != Some(i)
    }

    /// How many rows are currently on screen.
    pub fn visible(&self) -> usize {
        (0..self.count).filter(|&i| self.is_visible(i)).count()
    }

    /// The row the pointer is over, if any — published so a test can
    /// assert the highlight rather than the pixels under it.
    pub fn hot(&self) -> Option<usize> {
        self.hot
    }

    fn row_at(&self, y: f32) -> Option<usize> {
        if self.height <= 0.0 || y < 0.0 {
            return None;
        }
        let k = (y / TREE_ROW_H) as usize;
        (0..self.count).filter(|&i| self.is_visible(i)).nth(k)
    }
}

impl Widget for Tree {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    // Focusable (M57/A12), so the keyboard can reach it at all.
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = "tree";
    const SEEDS: &'static [Seed] =
        &[Seed::new(<Tree as Widget>::KIND, <Tree as Widget>::type_defaults, Fields::of(
            &[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::RADIUS,
                Fields::VISIBLE,
            ],
        )).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text).background(lumen_static::style::Role::Raised))];

    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(260.0),
            height: Length::Shrink,
            padding: Edges::all(8.0),
            background: Color(0xff, 0xff, 0xff, 0xff),
            color: Color(0x30, 0x3a, 0x4c, 0xff),
            font_size: 14.0,
            radius: 8.0,
            ..Style::default()
        }
    }
}

impl Node for Tree {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let content = Size {
            w: limits.max_w,
            h: self.visible() as f32 * TREE_ROW_H + me.padding.vertical(),
        };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        self.height = s.h;
        self.pad = me.padding;
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        // **Clipped to its own box** — a tree in a pane is the normal
        // case (a sidebar has a size), and a tree with more rows than
        // the pane is taller than the pane by definition: `layout`
        // resolves against the limits and the *rows* do not shrink. The
        // review measured 116 px of rows painted over the note below the
        // widget, on a fixture that happened to fit by 2 px
        // (M27/T27.11, finding 2). `Repeat` clips for the same reason
        // and by the same two calls.
        out.push_clip(r);
        let p = s.pad;
        let mut k = 0usize;
        for idx in 0..self.count {
            if !self.is_visible(idx) {
                continue;
            }
            let y = r[1] + p.1 + TREE_ROW_H * k as f32;
            let x = r[0] + p.0 + INDENT * self.rows[idx].depth as f32;
            // **Selection is the strong fill, hover a tint** — M57/B50:
            // the hover *was* the strong fill, and there was no selection.
            let selected = Some(idx) == self.selected;
            if selected {
                out.rounded_rect([r[0] + 4.0, y, r[2] - 4.0, y + TREE_ROW_H], s.fg, 5.0);
            } else if Some(idx) == self.hot {
                out.rounded_rect([r[0] + 4.0, y, r[2] - 4.0, y + TREE_ROW_H], Color(s.fg.0, s.fg.1, s.fg.2, 0x1c), 5.0);
            }
            let col = if selected { s.bg } else { s.fg };
            // A chevron only where there is something to expand.
            if self.has_children(idx) {
                let (cx0, mid, t) = (x + 5.0, y + TREE_ROW_H / 2.0, 3.5);
                if self.open[idx] {
                    out.line(Point { x: cx0 - t, y: mid - t / 2.0 },
                             Point { x: cx0, y: mid + t / 2.0 }, 2.0, col);
                    out.line(Point { x: cx0, y: mid + t / 2.0 },
                             Point { x: cx0 + t, y: mid - t / 2.0 }, 2.0, col);
                } else {
                    out.line(Point { x: cx0 - t / 2.0, y: mid - t },
                             Point { x: cx0 + t / 2.0, y: mid }, 2.0, col);
                    out.line(Point { x: cx0 + t / 2.0, y: mid },
                             Point { x: cx0 - t / 2.0, y: mid + t }, 2.0, col);
                }
            }
            out.text_weighted(
                [x + 16.0, y + TREE_ROW_H / 2.0 - s.font_size * 0.6, r[2] - p.2, y + TREE_ROW_H],
                self.rows[idx].label,
                s.font_size,
                s.font_weight,
                col,
            );
            k += 1;
        }
        out.pop_clip();
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // **The padding is read, not assumed.** `Tree::SEEDS`
            // declares it reads `PADDING`, so a sheet can set it — and a
            // hit test with `- 8.0` written into it would then pick a
            // different row from the one `paint` drew (M27/T27.11,
            // finding 14).
            // **`Hover`, not `PointerMove`.** A move is delivered only
            // under pointer capture, and capture follows a press — so
            // this arm, written as a move, could fire only while the
            // button was still held after a fold. Two widgets shipped
            // that way (M27/T27.11, finding 3).
            // The pointer left: no row is hot (M57/B50).
            Event::Leave => self.hot.take().is_some(),
            Event::Hover { .. } => {
                let was = self.hot;
                self.hot = self.row_at(cx.local.y - self.pad.top);
                was != self.hot
            }
            // A press selects the row, and folds or unfolds it if it has
            // children — a file explorer's click.
            Event::PointerDown { .. } => match self.row_at(cx.local.y - self.pad.top) {
                Some(idx) => {
                    self.select(idx);
                    self.chosen = true;
                    self.toggle(idx);
                    true
                }
                None => false,
            },
            // **The keyboard** — M57/B50. Up/Down walk the visible rows;
            // Right opens a closed row or steps into an open one; Left
            // closes an open row or steps out to the parent; Enter
            // chooses. At an end the key is declined, so an arrow can move
            // focus on through a group.
            Event::Key { key, down: true } if self.count > 0 => {
                let cur = self.selected.filter(|&i| i < self.count && self.is_visible(i));
                match (key, cur) {
                    (Key::Down | Key::Up, None) => self.select((0..self.count).find(|&k| self.is_visible(k)).unwrap_or(0)),
                    (Key::Down, Some(i)) => self.step(i, true).is_some_and(|k| self.select(k)),
                    (Key::Up, Some(i)) => self.step(i, false).is_some_and(|k| self.select(k)),
                    (Key::Right, Some(i)) if self.has_children(i) && !self.open[i] => self.toggle(i),
                    (Key::Right, Some(i)) if self.has_children(i) => self.select(i + 1),
                    (Key::Left, Some(i)) if self.has_children(i) && self.open[i] => self.toggle(i),
                    (Key::Left, Some(i)) => self.parent(i).is_some_and(|k| self.select(k)),
                    (Key::Enter, Some(_)) => {
                        self.chosen = true;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    lumen_static::leaf_common!(Tree, 0);

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Tree);
        node.set_size_of_set(self.count);
        // The visible labels, so a drive can assert a collapse without
        // reading pixels.
        let mut v = alloc::string::String::new();
        for i in 0..self.count {
            if self.is_visible(i) {
                if !v.is_empty() {
                    v.push(',');
                }
                v.push_str(self.rows[i].label);
            }
        }
        node.set_value(v);
    }
}

impl<S> lumen_static::event::Handlers<S> for Tree {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- DataGrid --------------------------------------------------------

/// The most columns one [`DataGrid`] shows.
pub const MAX_COLUMNS: usize = 8;

/// Part ids — unique within the grid, which is all they have to be since
/// M27/T27.0's descendant step.
pub const PART_ROWS: &str = "grid-rows";

/// A spreadsheet grid over rows the application binds.
///
/// **It owns no data.** Its rows are a [`lumen_static::repeat::Repeat`]
/// of the application's own row type, bound by the `fn` `Repeat` takes —
/// because a grid is a *view* of a table someone else holds, and copying
/// it in would be a second copy to keep in step. That is the opposite
/// call from `Chooser`, which owns its options, and the reason is the
/// same in both: who the data belongs to.
///
/// # A sheet, not a list
///
/// It read as a list — a header over rows with hairlines between them —
/// and was reported as one. It draws what a spreadsheet draws:
///
/// * **cells** — a rule at every column boundary, through the header
///   and the body, and one between rows;
/// * a **header band** and a **row-number gutter** ([`DataGrid::row_numbers`]),
///   tinted, meeting in a corner cell; the gutter follows the scroll;
/// * an **active cell**, outlined in the grid's `border_color` (the
///   palette's accent role), with its row number and column heading
///   tinted to match. A press on a cell makes it active; so do the
///   arrows, Home/End (first and last column), Ctrl+Home/End (first
///   and last row) and PageUp/PageDown, each scrolling the cell into
///   view. The grid is focusable for exactly this: D3 found its rows
///   unreachable from the keyboard (M57).
///
/// A row selected either way is reported once by
/// [`DataGrid::take_selected`], and [`DataGrid::active`] is the cell.
///
/// # Cells line up by construction
///
/// The body is handed a box exactly as wide as its columns and already
/// past the gutter, so a row whose `k`-th child is `FILL` gives it exactly
/// column `k` — [`DataGrid::column_width`]. The rules are drawn at those
/// boundaries, so **a cell pads its own text** by [`DataGrid::CELL_INSET`]:
/// the grid cannot inset a label inside the application's row, and a
/// grid that inset the whole body would draw its rules through the text.
pub struct DataGrid<R: Widget + Default + 'static> {
    columns: [&'static str; MAX_COLUMNS],
    column_count: usize,
    pub rows: lumen_static::repeat::Repeat<R>,
    /// Each column header's alignment: `Start` or `End`.
    pub align: [lumen_static::style::Align; MAX_COLUMNS],
    /// Draw the row-number gutter. On by default: it is most of what makes
    /// a grid read as a sheet.
    pub row_numbers: bool,
    /// Each header's measured width, for an `End` header.
    header_w: [f32; MAX_COLUMNS],
    /// The active cell's column; its row is `rows.current`.
    col: usize,
    /// The gutter's width from layout, and the body's height, which the
    /// keyboard scrolls against.
    gutter: f32,
    body_h: f32,
    /// Its own width from layout: `on_event` has no state array, and the
    /// column under a press is this width's arithmetic.
    width: f32,
}

impl<R: Widget + Default + 'static> Default for DataGrid<R> {
    fn default() -> Self {
        DataGrid::new(&[], 0, 24.0, |_, _| {})
    }
}

/// `n` in decimal, into `buf`: a row number, painted without allocating
/// (a paint is steady-state code — T29.5).
fn decimal(mut n: usize, buf: &mut [u8; 20]) -> &str {
    let mut at = buf.len();
    loop {
        at -= 1;
        buf[at] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    core::str::from_utf8(&buf[at..]).unwrap_or("")
}

impl<R: Widget + Default + 'static> DataGrid<R> {
    const ROWS: usize = 1;
    /// The header's height.
    const HEADER: f32 = 28.0;
    /// How far a cell's text sits from its column's rule — the header's,
    /// and what a row's cell should use for its own (see the type's docs).
    /// Also the strip at the right of the body where the scroll thumb runs.
    pub const CELL_INSET: f32 = 8.0;
    /// The narrowest row-number gutter.
    const MIN_GUTTER: f32 = 36.0;

    pub fn new(
        columns: &[&'static str],
        count: usize,
        row_extent: f32,
        bind: fn(&mut R, usize),
    ) -> Self {
        let mut rows = lumen_static::repeat::Repeat::new(count, row_extent, bind);
        rows.hover_rows = true;
        rows.select_rows = true;
        let mut g = DataGrid {
            columns: [""; MAX_COLUMNS],
            column_count: 0,
            rows,
            align: [lumen_static::style::Align::Start; MAX_COLUMNS],
            row_numbers: true,
            header_w: [0.0; MAX_COLUMNS],
            col: 0,
            gutter: 0.0,
            body_h: 0.0,
            width: 0.0,
        };
        for (i, c) in columns.iter().take(MAX_COLUMNS).enumerate() {
            g.columns[i] = c;
            g.column_count = i + 1;
        }
        g
    }

    /// How wide one column is, given the grid's own width: what is left
    /// after the gutter and the thumb's strip, divided evenly.
    ///
    /// **Published, because a row cannot line up with a number it
    /// cannot see.** The rows are the application's widget, bound by a
    /// plain `fn`; the review measured the `Size` column's values 72 px
    /// left of the `Size` header on a demo whose header and rows were
    /// each correct (M27/T27.11, finding 4). The body's box is exactly
    /// these columns, so `FILL` cells agree with it by construction, and
    /// `the_header_sits_over_a_row_that_follows_the_column_rule` pins it.
    pub fn column_width(&self, grid_w: f32) -> f32 {
        (grid_w - self.gutter - Self::CELL_INSET).max(0.0) / self.column_count.max(1) as f32
    }

    pub fn columns(&self) -> usize {
        self.column_count
    }

    /// The row a press or a key selected since the last call — and
    /// clears it.
    pub fn take_selected(&mut self) -> Option<usize> {
        self.rows.take_selected()
    }

    /// The selected row, drawn with the current-row tint.
    pub fn selected(&self) -> Option<usize> {
        self.rows.current
    }

    /// The active cell as `(row, column)`, once a row has been selected.
    pub fn active(&self) -> Option<(usize, usize)> {
        self.rows.current.map(|r| (r, self.col))
    }

    pub fn column(&self, i: usize) -> &'static str {
        self.columns[i.min(MAX_COLUMNS - 1)]
    }

    /// Make `(row, col)` the active cell, clamped to the sheet, and scroll
    /// it into view. Answers whether anything moved.
    fn go_to(&mut self, row: usize, col: usize) -> bool {
        if self.rows.count == 0 || self.column_count == 0 {
            return false;
        }
        let row = row.min(self.rows.count - 1);
        let col = col.min(self.column_count - 1);
        let moved = self.rows.current != Some(row) || self.col != col;
        self.col = col;
        self.rows.select(row);
        let scrolled = self.rows.scroll_into_view(row, self.body_h);
        moved || scrolled
    }

    /// The column under `x`, from the grid's own left edge.
    fn column_at(&self, x: f32, grid_w: f32) -> Option<usize> {
        let each = self.column_width(grid_w);
        let along = x - self.gutter;
        if each <= 0.0 || along < 0.0 {
            return None;
        }
        let k = (along / each) as usize;
        (k < self.column_count).then_some(k)
    }
}

pub struct GridTable<R>(core::marker::PhantomData<R>);

impl<R: Widget + Default + 'static> GridTable<R> {
    const N: usize = 1 + <lumen_static::repeat::Repeat<R> as Widget>::NODE_COUNT;
    const ARRAY: [NodeDesc; 2] = {
        // A `Repeat` is one table node whatever its rows, so a grid is
        // exactly two — the header, gutter and rules are this node's own
        // paint. Focusable, because the active cell moves by key.
        assert!(Self::N == 2, "a Repeat is one table node");
        let mut t = [NodeDesc::EMPTY; 2];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER | NodeDesc::FOCUSABLE);
        t[1] = NodeDesc::new(0, 0, 0, 0, 0).with_id(PART_ROWS);
        t
    };
    pub const SLICE: &'static [NodeDesc] = &Self::ARRAY;

    const SEED_ARRAY: [Seed; 2] = {
        let mut s = [Seed::WIDGET; 2];
        s[0] = Seed::new(<DataGrid<R> as Widget>::KIND, <DataGrid<R> as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::RADIUS,
                Fields::VISIBLE,
            ])).with_roles(lumen_static::style::SeedRoles::NONE
                .color(lumen_static::style::Role::Muted)
                .background(lumen_static::style::Role::Raised)
                // The active cell's outline — not a frame: the grid draws
                // its own frame in the rule colour, and `border_width` is
                // zero by default.
                .border(lumen_static::style::Role::Accent));
        s[1] = <lumen_static::repeat::Repeat<R> as Widget>::SEEDS[0];
        s
    };
    pub const SEED_SLICE: &'static [Seed] = &Self::SEED_ARRAY;
}

impl<R: Widget + Default + 'static> Widget for DataGrid<R> {
    const NODE_COUNT: usize = 2;
    const DEPTH: usize = 1 + <lumen_static::repeat::Repeat<R> as Widget>::DEPTH;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = GridTable::<R>::SLICE;
    const KIND: &'static str = "datagrid";
    const SEEDS: &'static [Seed] = GridTable::<R>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(200.0),
            background: Color(0xff, 0xff, 0xff, 0xff),
            color: Color(0x5b, 0x66, 0x7a, 0xff),
            border_color: Color(0x25, 0x63, 0xeb, 0xff),
            font_size: 13.0,
            font_weight: 600.0,
            radius: 8.0,
            ..Style::default()
        }
    }
}

impl<R: Widget + Default + Node + 'static> Node for DataGrid<R> {
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
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
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: limits.max_h },
        );
        // The gutter fits the largest row number, in the header's face.
        self.gutter = if self.row_numbers {
            let mut buf = [0u8; 20];
            let widest = cx.text.measure_weighted(decimal(self.rows.count.max(1), &mut buf), me.font_size, me.font_weight, f32::INFINITY).w;
            // Whole pixels: the rows are placed at a snapped x (M57/A15),
            // and the rules are drawn at the gutter's edge.
            lumen_static::num::ceil(widest + 2.0 * Self::CELL_INSET).max(Self::MIN_GUTTER)
        } else {
            0.0
        };
        // The rows get what is left under the header — **its resolved
        // height, not the limits it was offered**, which is the
        // asymmetry M27/T27.4 fixed inside `Repeat` and which a caller
        // has to respect too or the repeater cannot scroll.
        let inner = (s.w - self.gutter - Self::CELL_INSET).max(0.0);
        self.width = s.w;
        self.body_h = (s.h - Self::HEADER).max(0.0);
        let body = Constraints { min_w: inner, max_w: inner, min_h: 0.0, max_h: self.body_h };
        self.rows.layout(cx, body, style, st, i + Self::ROWS);
        for k in 0..self.column_count {
            if self.align[k] == lumen_static::style::Align::End {
                self.header_w[k] = cx.text.measure_weighted(self.columns[k], me.font_size, me.font_weight, f32::INFINITY).w;
            }
        }
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.rows.place(
            Point { x: origin.x + self.gutter, y: origin.y + Self::HEADER },
            style,
            st,
            i + Self::ROWS,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        let tint = |a: u8| Color(s.fg.0, s.fg.1, s.fg.2, a);
        let accent = |a: u8| Color(s.border_color.0, s.border_color.1, s.border_color.2, a);
        let rule = tint(0x33);
        let each = self.column_width(r[2] - r[0]);
        let (c0, body_top) = (r[0] + self.gutter, r[1] + Self::HEADER);
        // A column boundary, snapped as the cells' own positions are.
        let edge = |k: usize| lumen_static::num::round(c0 + each * k as f32);
        let right = edge(self.column_count);
        let b = lumen_static::ops::rect_of(&st[i + Self::ROWS]);
        let ext = self.rows.item_extent;
        // Which row is active, and where it is drawn, if it is on screen.
        let active = self.rows.current.filter(|&row| row < self.rows.count);
        let row_y = |row: usize| b[1] + row as f32 * ext - self.rows.scroll;

        out.rounded_rect(r, s.bg, s.radius);
        out.push_clip(r);
        // The header band and the gutter, one tint, meeting in the corner.
        out.rect([r[0], r[1], r[2], body_top], tint(0x10));
        if self.gutter > 0.0 {
            out.rect([r[0], body_top, c0, r[3]], tint(0x10));
        }
        // The active cell's heading and row number, tinted to match it.
        if let Some(row) = active {
            out.rect([edge(self.col), r[1], edge(self.col + 1), body_top], accent(0x26));
            if self.gutter > 0.0 {
                let y = row_y(row);
                out.push_clip([r[0], body_top, c0, r[3]]);
                out.rect([r[0], y, c0, y + ext], accent(0x26));
                out.pop_clip();
            }
        }
        for k in 0..self.column_count {
            let mut x = c0 + each * k as f32 + Self::CELL_INSET;
            if self.align[k] == lumen_static::style::Align::End {
                x = c0 + each * (k + 1) as f32 - Self::CELL_INSET - self.header_w[k];
            }
            out.text_weighted(
                [x, r[1] + Self::HEADER / 2.0 - s.font_size * 0.6,
                 x + each, r[1] + Self::HEADER],
                self.columns[k],
                s.font_size,
                s.font_weight,
                s.fg,
            );
        }
        out.pop_clip();

        self.rows.paint(st, i + Self::ROWS, out);

        // The row numbers and the rules, clipped below the header so a
        // scrolled row never draws over it.
        let below = [r[0], body_top, r[2], r[3]];
        out.push_clip(below);
        if ext > 0.0 && b[3] > b[1] {
            let first = (self.rows.scroll / ext) as usize;
            let mut k = first;
            while k < self.rows.count {
                let y = row_y(k);
                if y > r[3] {
                    break;
                }
                if self.gutter > 0.0 {
                    let mut buf = [0u8; 20];
                    let strong = active == Some(k);
                    out.text_weighted(
                        [r[0] + Self::CELL_INSET, y + ext / 2.0 - s.font_size * 0.6, c0, y + ext],
                        decimal(k + 1, &mut buf),
                        s.font_size,
                        if strong { 700.0 } else { 400.0 },
                        s.fg,
                    );
                }
                // The rule under row k, across the gutter and the cells —
                // one whole pixel, as every rule here is (X10: a 1 px line
                // on a half-pixel edge renders 2 px wide and pale).
                let yb = lumen_static::num::round(y + ext);
                out.rect([r[0], yb - 1.0, right, yb], tint(0x22));
                k += 1;
            }
        }
        out.pop_clip();
        // The column rules, through the header and the body; the first is
        // the gutter's edge, the last the sheet's. Each in the pixel just
        // inside its boundary's left, so the last one closes the sheet.
        for k in 0..=self.column_count {
            if k == 0 && self.gutter <= 0.0 {
                continue;
            }
            let x = edge(k);
            out.rect([x - 1.0, r[1], x, r[3]], rule);
        }
        // The rule under the header, so it reads as a header rather than as
        // a first row.
        out.rect([r[0], body_top - 1.0, right, body_top], rule);

        // The active cell, over its row's tint and inside the body.
        if let Some(row) = active {
            let y = row_y(row);
            out.push_clip(below);
            let (y0, y1) = (lumen_static::num::round(y), lumen_static::num::round(y + ext));
            out.stroked_rect([edge(self.col) - 1.0, y0 - 1.0, edge(self.col + 1), y1], accent(0xff), 0.0, 2.0);
            out.pop_clip();
        }

        // The thumb, in the strip right of the cells, while the rows
        // overflow: its length the viewport's share of the content, its
        // position the scroll's.
        let content = self.rows.count as f32 * ext;
        let view = b[3] - b[1];
        if content > view + 0.5 && view > 0.0 {
            let len = (view * view / content).max(24.0).min(view);
            let y = b[1] + (view - len) * (self.rows.scroll / (content - view)).clamp(0.0, 1.0);
            let x = right + (Self::CELL_INSET - 4.0) / 2.0;
            out.rounded_rect([x, y + 2.0, x + 4.0, y + len - 2.0], tint(0x70), 2.0);
        }
        // The frame, in the rules' colour, and an author's border over it.
        out.stroked_rect(r, rule, s.radius, 1.0);
        lumen_static::ops::border(st, i, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // The rows first. The header and gutter answer as the grid itself,
        // so a wheel over them reaches `on_event` and scrolls the rows
        // (M57/B51), and a press on a heading picks its column.
        self.rows.hit(st, i + Self::ROWS, x, y).or(Some(lumen_static::Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        }))
    }

    /// The wheel over the header, a press on a cell or a heading, and the
    /// keys that move the active cell.
    ///
    /// A press on a cell reaches here **bubbled** from the rows, which
    /// record the row and decline it (a control inside a row still gets
    /// its press first); the pass hands every node it climbs to its own
    /// `local`, so the column is this node's arithmetic.
    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        let w = self.width;
        match ev {
            Event::Scroll { .. } if cx.target == cx.node => self.rows.on_event(ev, cx),
            // **The row from this node's own `local`, not `cx.row`**: the
            // row is the repeater's coordinate and does not survive the
            // climb out of it, so the grid reads its own — the body starts
            // under the header and is scrolled by the rows' offset.
            Event::PointerDown { .. } => {
                let Some(col) = self.column_at(cx.local.x, w) else { return false };
                if cx.local.y < Self::HEADER {
                    // A heading: that column, in the active row (or the first).
                    let row = self.rows.current.unwrap_or(0);
                    return self.go_to(row, col);
                }
                let ext = self.rows.item_extent;
                if ext <= 0.0 {
                    return false;
                }
                let row = ((cx.local.y - Self::HEADER + self.rows.scroll) / ext) as usize;
                if row >= self.rows.count {
                    return false;
                }
                // The rows recorded this row on the way up; the column is
                // what only the grid can know.
                let moved = self.col != col || self.rows.current != Some(row);
                self.col = col;
                moved
            }
            Event::Key { key, down: true } => {
                if self.rows.count == 0 || self.column_count == 0 {
                    return false;
                }
                let ctrl = cx.mods.has(lumen_static::event::Mods::CTRL);
                let (row, col) = match self.rows.current {
                    Some(r) => (r, self.col),
                    // From nothing, any move lands on the first cell.
                    None => return matches!(key, Key::Up | Key::Down | Key::Left | Key::Right | Key::Home | Key::End | Key::PageUp | Key::PageDown) && self.go_to(0, 0),
                };
                let page = if self.rows.item_extent > 0.0 { ((self.body_h / self.rows.item_extent) as usize).max(1) } else { 1 };
                let last_row = self.rows.count - 1;
                let (row, col) = match key {
                    Key::Up => (row.saturating_sub(1), col),
                    Key::Down => ((row + 1).min(last_row), col),
                    Key::Left => (row, col.saturating_sub(1)),
                    Key::Right => (row, col + 1),
                    Key::Home if ctrl => (0, col),
                    Key::End if ctrl => (last_row, col),
                    Key::Home => (row, 0),
                    Key::End => (row, self.column_count - 1),
                    Key::PageUp => (row.saturating_sub(page), col),
                    Key::PageDown => ((row + page).min(last_row), col),
                    _ => return false,
                };
                self.go_to(row, col)
            }
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == 0 {
            self
        } else {
            &self.rows
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == 0 {
            self
        } else {
            &mut self.rows
        }
    }

    fn arena_count(&self) -> usize {
        self.rows.arena_count()
    }

    fn arena_mut(&mut self, a: usize) -> Option<lumen_static::arena::ArenaMut<'_>> {
        self.rows.arena_mut(a)
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Grid);
        let mut v = alloc::string::String::new();
        for k in 0..self.column_count {
            if !v.is_empty() {
                v.push(',');
            }
            v.push_str(self.columns[k]);
        }
        node.set_value(v);
        node.set_row_count(self.rows.count);
        node.set_column_count(self.column_count);
    }
}

/// Rows route through the repeater, which is where `EventCtx::row` comes
/// from.
impl<R, S> lumen_static::event::Handlers<S> for DataGrid<R>
where
    R: Widget + Default + 'static + lumen_static::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.rows
            .handler(lumen_static::event::NodeSel { i: sel.i - Self::ROWS, inner: sel.inner })
    }
}
