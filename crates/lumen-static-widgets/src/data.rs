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
pub struct Pagination {
    pages: usize,
    page: usize,
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
        Pagination { pages: 1, page: 0, width: 0.0, widths: [0.0; MAX_PAGES] }
    }
}

impl Pagination {
    pub fn new(pages: usize) -> Self {
        Pagination { pages: pages.max(1), page: 0, width: 0.0, widths: [0.0; MAX_PAGES] }
    }

    pub fn pages(&self) -> usize {
        self.pages
    }

    pub fn page(&self) -> usize {
        self.page
    }

    /// Go to `p`, clamped. Returns whether it moved.
    pub fn go(&mut self, p: usize) -> bool {
        let p = p.min(self.pages - 1);
        let moved = p != self.page;
        self.page = p;
        moved
    }

    pub fn next_page(&mut self) -> bool {
        self.go(self.page + 1)
    }

    pub fn prev_page(&mut self) -> bool {
        self.go(self.page.saturating_sub(1))
    }

    /// The window of page numbers shown: at most [`MAX_PAGES`], centred
    /// on the current page and clamped to the ends.
    ///
    /// **Centred, not paged in blocks.** A block-paged strip jumps under
    /// the finger — you press "5" and the numbers shift so that 5 is now
    /// somewhere else — which is the one thing every pagination gets
    /// wrong and nobody writes a test for.
    pub fn window(&self) -> (usize, usize) {
        let n = self.pages.min(MAX_PAGES);
        let half = n / 2;
        let first = self.page.saturating_sub(half).min(self.pages - n);
        (first, n)
    }

    /// Slots across the control: `<`, the page window, `>`.
    fn slots(&self) -> usize {
        self.window().1 + 2
    }

    fn slot_at(&self, x: f32) -> Option<usize> {
        if self.width <= 0.0 {
            return None;
        }
        let each = self.width / self.slots() as f32;
        if each <= 0.0 {
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
            ]),)];

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
    /// Arrows move the page, so they never leave this control
    /// (M44/T44.1; [`Node::owns_arrows`]).
    fn owns_arrows(&self) -> bool {
        true
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
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: 36.0 },
        );
        self.width = s.w;
        let (first, n) = self.window();
        for k in 0..n {
            let label = PageLabel::new(first + k + 1);
            self.widths[k] = cx
                .text
                .measure_weighted(
                    label.as_str(),
                    me.font_size,
                    if first + k == self.page { 600.0 } else { me.font_weight },
                    limits.max_w,
                )
                .w;
        }
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    /// `layout` with nothing kept — M41/T41.7. `layout` caches the width
    /// `on_event` divides into slots and each number's measured width,
    /// which `paint` centres by; a probe measures at the audit's 1e6 px
    /// and with whatever measurer the audit holds (the block stand-in, in
    /// a window that installs none). Kept, the next frame under
    /// `Boundaries` — which does not lay an unmarked pagination out —
    /// painted the probe's centres: the live oracle's first divergence
    /// in the showcase (`docs/plan-boundaries-repair.md` §6).
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let kept = (self.width, self.widths);
        let size = self.layout(cx, limits, style, st, i);
        (self.width, self.widths) = kept;
        size
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        lumen_static::ops::focus_ring(st, i, out);
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        let (first, n) = self.window();
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
        for k in 0..n {
            let p = first + k;
            let x = r[0] + each * (k as f32 + 1.0);
            if p == self.page {
                out.rounded_rect([x + 3.0, r[1] + 4.0, x + each - 3.0, r[3] - 4.0], s.fg, 6.0);
            }
            let col = if p == self.page { s.bg } else { s.fg };
            // Page numbers are 1-based on screen and 0-based in the
            // field: every user-facing pagination counts from one, and
            // every array does not.
            let label = PageLabel::new(p + 1);
            // Centred in the slot, from the width `layout` measured —
            // the same centre the pill above uses, so the two agree by
            // construction rather than by coincidence.
            let tw = self.widths[k].min(each);
            let tx = x + (each - tw) / 2.0;
            out.text_weighted(
                [tx, r[1] + h / 2.0 - s.font_size * 0.6, tx + tw, r[3]],
                label.as_str(),
                s.font_size,
                if p == self.page { 600.0 } else { s.font_weight },
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
                Some(k) => {
                    let (first, _) = self.window();
                    self.go(first + k - 1);
                    true
                }
                None => false,
            },
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
    pub(crate) count: usize,
    /// Whether each row is expanded. A row with no children is a leaf
    /// and this is ignored for it.
    pub(crate) open: [bool; MAX_TREE_NODES],
    /// The row the pointer is over.
    hot: Option<usize>,
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

    /// The row a row `i` sits under, if any: the last row above it with a
    /// smaller depth.
    fn parent_of(&self, i: usize) -> Option<usize> {
        let d = self.rows[i].depth;
        (0..i).rev().find(|&k| self.rows[k].depth < d)
    }

    /// **The keyboard's walk through the tree** — M44/T44.10, the
    /// platform's tree-view keys.
    ///
    /// Up and Down move the lit row (the same `hot` the pointer lights)
    /// through the *visible* rows, Home and End to the ends. Right opens
    /// a closed parent, or on an open one steps to its first child; Left
    /// closes an open parent, or steps to the row's parent. Enter and
    /// Space fold or unfold the lit row, as a press on it does. Returns
    /// whether it did anything.
    pub fn key(&mut self, key: Key) -> bool {
        let shown: [Option<usize>; MAX_TREE_NODES] = {
            let mut a = [None; MAX_TREE_NODES];
            let mut n = 0;
            for i in 0..self.count {
                if self.is_visible(i) {
                    a[n] = Some(i);
                    n += 1;
                }
            }
            a
        };
        let n = shown.iter().flatten().count();
        if n == 0 {
            return false;
        }
        let at = self.hot.and_then(|h| shown[..n].iter().position(|&x| x == Some(h)));
        let lit = |k: usize| shown[k];
        let next = match key {
            Key::Down => lit(at.map_or(0, |k| (k + 1).min(n - 1))),
            Key::Up => lit(at.map_or(0, |k| k.saturating_sub(1))),
            Key::Home => lit(0),
            Key::End => lit(n - 1),
            Key::Right | Key::Left | Key::Enter | Key::Space => {
                let Some(h) = self.hot.filter(|&h| h < self.count) else {
                    return false;
                };
                return match key {
                    Key::Right if self.has_children(h) && !self.open[h] => self.toggle(h),
                    Key::Right if self.has_children(h) => {
                        self.hot = Some(h + 1);
                        true
                    }
                    Key::Left if self.has_children(h) && self.open[h] => self.toggle(h),
                    Key::Left => match self.parent_of(h) {
                        Some(p) => {
                            self.hot = Some(p);
                            true
                        }
                        None => false,
                    },
                    Key::Enter | Key::Space => self.toggle(h),
                    _ => false,
                };
            }
            _ => return false,
        };
        let moved = next != self.hot;
        self.hot = next;
        moved
    }
}

impl Widget for Tree {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    /// **A Tab stop** (M44/T44.10): a tree is walked with its own keys,
    /// so a keyboard has to be able to reach it.
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
        ))];

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
        // The ring, now that a tree can hold focus (M44/T44.10).
        lumen_static::ops::focus_ring(st, i, out);
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
            if Some(idx) == self.hot {
                out.rounded_rect([r[0] + 4.0, y, r[2] - 4.0, y + TREE_ROW_H], s.fg, 5.0);
            }
            let col = if Some(idx) == self.hot { s.bg } else { s.fg };
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
            Event::Hover { .. } => {
                let was = self.hot;
                self.hot = self.row_at(cx.local.y - self.pad.top);
                was != self.hot
            }
            Event::PointerDown { .. } => match self.row_at(cx.local.y - self.pad.top) {
                Some(idx) => self.toggle(idx),
                None => false,
            },
            // Focused: the tree-view keys (M44/T44.10).
            Event::Key { key, down: true } => self.key(*key),
            _ => false,
        }
    }

    /// Arrows walk and fold the tree (M44/T44.10).
    fn owns_arrows(&self) -> bool {
        true
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

/// A header strip over rows the application binds.
///
/// **It owns no data.** Its rows are a [`lumen_static::repeat::Repeat`]
/// of the application's own row type, bound by the `fn` `Repeat` takes —
/// because a grid is a *view* of a table someone else holds, and copying
/// it in would be a second copy to keep in step. That is the opposite
/// call from `Chooser`, which owns its options, and the reason is the
/// same in both: who the data belongs to.
pub struct DataGrid<R: Widget + Default + 'static> {
    columns: [&'static str; MAX_COLUMNS],
    column_count: usize,
    pub rows: lumen_static::repeat::Repeat<R>,
}

impl<R: Widget + Default + 'static> Default for DataGrid<R> {
    fn default() -> Self {
        DataGrid {
            columns: [""; MAX_COLUMNS],
            column_count: 0,
            rows: lumen_static::repeat::Repeat::new(0, 24.0, |_, _| {}),
        }
    }
}

impl<R: Widget + Default + 'static> DataGrid<R> {
    const ROWS: usize = 1;
    /// The header's height.
    const HEADER: f32 = 28.0;
    /// How far the grid's **content** — header and rows alike — is
    /// inset from its own edge.
    ///
    /// The inset is the grid's, not the cell's: it insets the header and
    /// hands the rows a box already inset by the same amount, so column
    /// k starts at one x for both. The alternative — the header insets
    /// itself and every row is expected to inset its cells to match —
    /// is what shipped in T27.10, and the `Size` values sat 72 px left
    /// of the `Size` heading (M27/T27.11, finding 4).
    pub const CELL_INSET: f32 = 8.0;

    pub fn new(
        columns: &[&'static str],
        count: usize,
        row_extent: f32,
        bind: fn(&mut R, usize),
    ) -> Self {
        let mut g = DataGrid {
            columns: [""; MAX_COLUMNS],
            column_count: 0,
            rows: lumen_static::repeat::Repeat::new(count, row_extent, bind),
        };
        for (i, c) in columns.iter().take(MAX_COLUMNS).enumerate() {
            g.columns[i] = c;
            g.column_count = i + 1;
        }
        g
    }

    /// How wide one column is, given the grid's own width.
    ///
    /// **Published, because a row cannot line up with a number it
    /// cannot see.** A grid divides its header evenly; its *rows* are
    /// the application's widget, bound by a plain `fn`, and nothing
    /// reconciles the two — the review measured the `Size` column's
    /// values 72 px left of the `Size` header, on a demo whose header
    /// was correct and whose rows were correct and which together were
    /// wrong (M27/T27.11, finding 4).
    ///
    /// A row lines up when its k-th child takes exactly one column —
    /// which a `row` container does for itself if its children are
    /// `FILL`, because the rows are handed a box already inset and
    /// already divided. `the_header_sits_over_a_row_that_follows_the_column_rule`
    /// pins it, and the showcase's `GridRow` is two `FILL` cells and no
    /// stylesheet rule at all.
    pub fn column_width(&self, grid_w: f32) -> f32 {
        (grid_w - 2.0 * Self::CELL_INSET).max(0.0) / self.column_count.max(1) as f32
    }

    pub fn columns(&self) -> usize {
        self.column_count
    }

    pub fn column(&self, i: usize) -> &'static str {
        self.columns[i.min(MAX_COLUMNS - 1)]
    }
}

pub struct GridTable<R>(core::marker::PhantomData<R>);

impl<R: Widget + Default + 'static> GridTable<R> {
    const N: usize = 1 + <lumen_static::repeat::Repeat<R> as Widget>::NODE_COUNT;
    const ARRAY: [NodeDesc; 2] = {
        // A `Repeat` is one table node whatever its rows, so a grid is
        // exactly two — the header is this node's own paint.
        assert!(Self::N == 2, "a Repeat is one table node");
        let mut t = [NodeDesc::EMPTY; 2];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        // **The rows are a Tab stop** (M44/T44.10): focused, the
        // `Repeat` keeps an active row the arrow and page keys move and
        // scroll into view, and Enter or Space runs its handler.
        t[1] = NodeDesc::new(0, 0, 0, 0, NodeDesc::FOCUSABLE).with_id(PART_ROWS);
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
            ]));
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
        // The rows get what is left under the header — **its resolved
        // height, not the limits it was offered**, which is the
        // asymmetry M27/T27.4 fixed inside `Repeat` and which a caller
        // has to respect too or the repeater cannot scroll.
        let inner = (s.w - 2.0 * Self::CELL_INSET).max(0.0);
        let body = Constraints {
            min_w: inner,
            max_w: inner,
            min_h: 0.0,
            max_h: (s.h - Self::HEADER).max(0.0),
        };
        self.rows.layout(cx, body, style, st, i + Self::ROWS);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.rows.place(
            Point { x: origin.x + Self::CELL_INSET, y: origin.y + Self::HEADER },
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
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        if self.column_count > 0 {
            let each = self.column_width(r[2] - r[0]);
            for k in 0..self.column_count {
                let x = r[0] + Self::CELL_INSET + each * k as f32;
                out.text_weighted(
                    [x, r[1] + Self::HEADER / 2.0 - s.font_size * 0.6,
                     x + each, r[1] + Self::HEADER],
                    self.columns[k],
                    s.font_size,
                    s.font_weight,
                    s.fg,
                );
            }
            // A rule under the header, so it reads as a header rather
            // than as a first row.
            out.rect([r[0], r[1] + Self::HEADER - 1.0, r[2], r[1] + Self::HEADER],
                     Color(s.fg.0, s.fg.1, s.fg.2, 0x33));
        }
        self.rows.paint(st, i + Self::ROWS, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // The rows first; the header is not a target (sorting is not
        // built, and a header that swallowed a press would be one that
        // did nothing).
        self.rows.hit(st, i + Self::ROWS, x, y)
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
