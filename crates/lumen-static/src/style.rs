//! Computed style, in a parallel array — plan M1
//! (`docs/layout-and-storage-decisions.md` §3).
//!
//! Style lives *beside* the tree, not inside the widgets: a
//! `[Style; NODE_COUNT]` keyed by node index, exactly like `NodeState`.
//! Widgets stay small (style on every widget would make
//! `size_of::<Label>()` explode and turn §3's "values in the fields"
//! into "everything in the fields"), allocation stays at zero, and the
//! existing change journal diffs it for free.
//!
//! Computed style is *derived*: it is recomputed by the cascade, never
//! migrated across a hot reload.

use crate::desc::NodeDesc;

/// How a node claims space along one axis — iced's model, which needs no
/// iteration to resolve (`docs/layout-and-storage-decisions.md` §2).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Length {
    /// Exactly this many pixels.
    Fixed(f32),
    /// The node's own content size.
    #[default]
    Shrink,
    /// Share the remainder with other fills, in this ratio.
    /// `Fill` is `FillPortion(1)`.
    FillPortion(u16),
}

impl Length {
    pub const FILL: Length = Length::FillPortion(1);

    #[inline]
    pub fn portion(&self) -> u16 {
        match self {
            Length::FillPortion(k) => *k,
            _ => 0,
        }
    }

    #[inline]
    pub fn is_fill(&self) -> bool {
        matches!(self, Length::FillPortion(_))
    }
}

/// Per-side lengths, for padding.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Edges {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Edges {
    pub const ZERO: Edges = Edges { left: 0.0, right: 0.0, top: 0.0, bottom: 0.0 };

    pub const fn all(v: f32) -> Edges {
        Edges { left: v, right: v, top: v, bottom: v }
    }

    #[inline]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    #[inline]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Cross-axis alignment of a container's children.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    /// Stretch to the container's cross size.
    Fill,
}

/// One node's computed style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub width: Length,
    pub height: Length,
    pub padding: Edges,
    /// Gap between a container's children.
    pub spacing: f32,
    /// Text colour.
    pub color: crate::display::Color,
    /// Fill behind this node. `TRANSPARENT` paints nothing.
    pub background: crate::display::Color,
    /// Corner radius for the background, in pixels.
    pub radius: f32,
    /// Text size in pixels.
    pub font_size: f32,
    /// Text weight, CSS-style: 400 regular, 700 bold.
    pub font_weight: f32,
    /// Drop shadow spread in pixels; 0 is none.
    pub shadow: f32,
    /// How much darker this node's background goes while the pointer is
    /// down on it, 0.0–1.0 (C6). 0 is no feedback.
    ///
    /// A scalar rather than a whole second `Patch` per state: pressed
    /// and hovered affordances are almost always a shade of the resting
    /// colour, and a `Style` is stored per node — doubling its size to
    /// express the rare case would cost every node in every tree. A
    /// control needing more than a shade reads `NodeState::pressed` in
    /// its own `paint`.
    pub pressed_dim: f32,
    /// The same while the pointer is merely over it.
    pub hover_dim: f32,
    pub align: Align,
    /// Whether this node and everything under it participates in the
    /// frame.
    ///
    /// `false` means: the subtree resolves to zero size, is not
    /// cross-stretched by its parent, records no drawing commands, and
    /// fails every hit test. All four are enforced —
    /// `crates/lumen-static/tests/visible.rs` asserts each clause of
    /// this sentence for a hidden container *and* a hidden leaf.
    ///
    /// Until 2026-09-02 none of them were. No leaf read this field at
    /// all, and the derive zeroed only the container's own slot, so
    /// descendants kept the previous frame's geometry and were painted
    /// at it. `Option`'s separate `hide()` path did the right thing, so
    /// two mechanisms disagreed. `docs/plan-review-fixes.md` F1.6.
    pub visible: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            padding: Edges::ZERO,
            spacing: 0.0,
            color: crate::display::Color::BLACK,
            background: crate::display::Color::TRANSPARENT,
            radius: 0.0,
            font_size: 16.0,
            font_weight: 400.0,
            shadow: 0.0,
            pressed_dim: 0.0,
            hover_dim: 0.0,
            align: Align::Start,
            visible: true,
        }
    }
}

/// A widget kind's own defaults, before any rule applies. Emitted by the
/// derive from `#[widget(...)]` and returned by leaves.
pub type Defaults = Style;

// ---- selectors and rules ----------------------------------------------

/// What a rule matches on. Deliberately small: id, class and kind are the
/// three axes Lumen's `.lss` cascade uses too, and they are all static
/// per node, so matching is a table scan with no allocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selector {
    /// `#save` — an author id.
    Id(&'static str),
    /// `.row` — a class.
    Class(&'static str),
    /// `label` — a widget kind name.
    /// A widget kind, as [`Node::kind_name`](crate::widget::Node::kind_name)
    /// reports it — **and the two are not spelled alike**. Built-in
    /// leaves are lowercase (`label`, `button`, `fixed`, `spacer`);
    /// derived containers use the struct's own ident (`Ui`, `Controls`).
    /// So `Selector::Kind("label")` and `Selector::Kind("Ui")` are both
    /// correct. Documented nowhere until 2026-09-02
    /// (`docs/plan-review-fixes.md` F7.5).
    Kind(&'static str),
    /// Everything.
    Any,
}

impl Selector {
    /// Specificity, highest wins: id > class > kind > any.
    #[inline]
    pub fn specificity(&self) -> u8 {
        match self {
            Selector::Id(_) => 3,
            Selector::Class(_) => 2,
            Selector::Kind(_) => 1,
            Selector::Any => 0,
        }
    }
}

/// What a rule changes. `None` fields leave the cascaded value alone, so
/// rules compose rather than overwrite.
#[derive(Clone, Copy, Debug, Default)]
pub struct Patch {
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub padding: Option<Edges>,
    pub spacing: Option<f32>,
    pub align: Option<Align>,
    pub visible: Option<bool>,
    pub color: Option<crate::display::Color>,
    pub background: Option<crate::display::Color>,
    pub radius: Option<f32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub shadow: Option<f32>,
    pub pressed_dim: Option<f32>,
    pub hover_dim: Option<f32>,
}

impl Patch {
    pub const fn new() -> Patch {
        Patch {
            width: None,
            height: None,
            padding: None,
            spacing: None,
            align: None,
            visible: None,
            color: None,
            background: None,
            radius: None,
            font_size: None,
            font_weight: None,
            shadow: None,
            pressed_dim: None,
            hover_dim: None,
        }
    }

    pub const fn width(mut self, l: Length) -> Self {
        self.width = Some(l);
        self
    }
    pub const fn height(mut self, l: Length) -> Self {
        self.height = Some(l);
        self
    }
    pub const fn padding(mut self, e: Edges) -> Self {
        self.padding = Some(e);
        self
    }
    pub const fn spacing(mut self, v: f32) -> Self {
        self.spacing = Some(v);
        self
    }
    pub const fn align(mut self, a: Align) -> Self {
        self.align = Some(a);
        self
    }
    pub const fn visible(mut self, v: bool) -> Self {
        self.visible = Some(v);
        self
    }

    pub const fn color(mut self, c: crate::display::Color) -> Self {
        self.color = Some(c);
        self
    }

    pub const fn background(mut self, c: crate::display::Color) -> Self {
        self.background = Some(c);
        self
    }

    pub const fn radius(mut self, r: f32) -> Self {
        self.radius = Some(r);
        self
    }

    pub const fn font_size(mut self, px: f32) -> Self {
        self.font_size = Some(px);
        self
    }

    pub const fn font_weight(mut self, w: f32) -> Self {
        self.font_weight = Some(w);
        self
    }

    pub const fn shadow(mut self, spread: f32) -> Self {
        self.shadow = Some(spread);
        self
    }

    /// How much darker the background goes while pressed (0.0–1.0).
    pub const fn pressed_dim(mut self, v: f32) -> Self {
        self.pressed_dim = Some(v);
        self
    }

    /// The same while hovered.
    pub const fn hover_dim(mut self, v: f32) -> Self {
        self.hover_dim = Some(v);
        self
    }

    #[inline]
    fn apply_to(&self, s: &mut Style) {
        if let Some(v) = self.width {
            s.width = v;
        }
        if let Some(v) = self.height {
            s.height = v;
        }
        if let Some(v) = self.padding {
            s.padding = v;
        }
        if let Some(v) = self.spacing {
            s.spacing = v;
        }
        if let Some(v) = self.align {
            s.align = v;
        }
        if let Some(v) = self.visible {
            s.visible = v;
        }
        if let Some(v) = self.color {
            s.color = v;
        }
        if let Some(v) = self.background {
            s.background = v;
        }
        if let Some(v) = self.radius {
            s.radius = v;
        }
        if let Some(v) = self.font_size {
            s.font_size = v;
        }
        if let Some(v) = self.font_weight {
            s.font_weight = v;
        }
        if let Some(v) = self.pressed_dim {
            s.pressed_dim = v;
        }
        if let Some(v) = self.hover_dim {
            s.hover_dim = v;
        }
        if let Some(v) = self.shadow {
            s.shadow = v;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub selector: Selector,
    pub patch: Patch,
}

impl Rule {
    pub const fn new(selector: Selector, patch: Patch) -> Rule {
        Rule { selector, patch }
    }
}

/// A stylesheet is just a slice of rules — a `.lss` parser (parity Phase
/// C) would produce one.
pub type Stylesheet<'a> = &'a [Rule];

fn matches(sel: &Selector, d: &NodeDesc, kind: &str) -> bool {
    match sel {
        Selector::Any => true,
        Selector::Kind(k) => kind == *k,
        Selector::Id(id) => d.wid == *id,
        Selector::Class(c) => d.classes.contains(c),
    }
}

/// Resolve every node's style: kind defaults first, then rules in
/// ascending specificity, then declaration order within a specificity.
///
/// One flat pass over the table — the cascade is the only place that
/// still walks the table linearly, and it re-runs only when the sheet or
/// the tree changes, not every frame.
/// **When to call this.** Not per frame. The result depends only on the
/// stylesheet, the table, and each kind's `defaults()` — and
/// [`Node::defaults`](crate::widget::Node::defaults) is required to
/// depend on the type alone, never on instance state. So the computed
/// styles change only when the sheet is replaced or the tree's shape is
/// (a hot reload), which is exactly when
/// [`Host::set_stylesheet`](../../lumen_static_shell/struct.Host.html)
/// and `Host::new` run it.
///
/// A `pass::frame` helper used to cascade and lay out together, on the
/// theory that the two belong in one step. It had zero callers — every
/// host and test open-coded the pair — and once `defaults()` became
/// state-independent it was not merely unused but misleading, since it
/// implied the cascade had to be re-run for a frame to be correct. It
/// was deleted rather than wired up. `docs/plan-review-fixes.md` F3.3.
pub fn cascade(
    table: &[NodeDesc],
    defaults: &dyn Fn(usize) -> Defaults,
    kind_of: &dyn Fn(usize) -> &'static str,
    sheet: Stylesheet,
    out: &mut [Style],
) {
    cascade_range(table, defaults, kind_of, sheet, out, 0..table.len());
}

/// [`cascade`], restricted to a range of the table.
fn cascade_range(
    table: &[NodeDesc],
    defaults: &dyn Fn(usize) -> Defaults,
    kind_of: &dyn Fn(usize) -> &'static str,
    sheet: Stylesheet,
    out: &mut [Style],
    range: core::ops::Range<usize>,
) {
    debug_assert_eq!(table.len(), out.len());
    for (i, d) in table.iter().enumerate().skip(range.start).take(range.len()) {
        let mut s = defaults(i);
        let kind = kind_of(i);
        for spec in 0..=3u8 {
            for r in sheet {
                if r.selector.specificity() == spec && matches(&r.selector, d, kind) {
                    r.patch.apply_to(&mut s);
                }
            }
        }
        out[i] = s;
    }
}

/// The half-open index range of the subtree rooted at `root`.
///
/// The table is DFS pre-order, so a subtree is *contiguous* — the
/// property that makes a scoped cascade a slice rather than a walk. The
/// end is the root's next sibling, or, when it has none, the next
/// sibling of the nearest ancestor that has one.
pub fn subtree_range(table: &[NodeDesc], root: usize) -> core::ops::Range<usize> {
    let mut i = root;
    loop {
        let d = &table[i];
        if d.next_sibling != 0 {
            return root..d.next_sibling as usize;
        }
        if d.parent == crate::desc::NO_PARENT {
            return root..table.len();
        }
        i = d.parent as usize;
    }
}

/// The main-table index of the node carrying `#[widget_id(wid)]`.
///
/// A table scan — 901 pointer comparisons on the largest fixture, next
/// to the ~36 µs a full cascade costs there
/// (`docs/results-cascade-cost.md`) — so an app may address a subtree by
/// the name it already wrote rather than by an index arithmetic it would
/// have to keep in step with the struct.
pub fn index_of_wid(table: &[NodeDesc], wid: &str) -> Option<usize> {
    // A node without `#[widget_id]` carries `""`, so an empty argument
    // would match the first unnamed node — which is usually the root,
    // and would restyle the whole tree. Nothing is named `""`.
    if wid.is_empty() {
        return None;
    }
    table.iter().position(|d| d.wid == wid)
}

/// Re-cascade one subtree: the node at `root` and its descendants.
///
/// **Sound because every [`Selector`] is node-local.** `Id`, `Class`,
/// `Kind` and `Any` all read the node's own descriptor and kind; none
/// reads an ancestor, a sibling or a position. So a node's resolved
/// style is a function of that node and the sheet, and re-resolving a
/// contiguous range cannot disagree with re-resolving the table. A
/// descendant combinator would end that property, and adding one means
/// revisiting this function in the same commit.
///
/// **The caller's contract:** `sheet` must differ from the one last
/// cascaded only in rules that match *inside* the subtree. It is the one
/// thing this cannot check cheaply, so in debug builds it checks it
/// expensively — a full cascade is run and every style outside the
/// range is compared. A release build skips that; a test does not, which
/// is where the mistake will be caught.
pub fn cascade_subtree<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    out: &mut [Style],
    root: usize,
) {
    let table = W::SUB_TABLE;
    let range = subtree_range(table, root);

    #[cfg(debug_assertions)]
    let witness: Vec<Style> = {
        let mut full = out.to_vec();
        {
            let t: &W = tree;
            cascade(table, &|i| t.node(i).defaults(), &|i| t.node(i).kind_name(), sheet, &mut full);
        }
        full
    };

    {
        let t: &W = tree;
        cascade_range(
            table,
            &|i| t.node(i).defaults(),
            &|i| t.node(i).kind_name(),
            sheet,
            out,
            range.clone(),
        );
    }
    for i in range.clone() {
        cascade_arenas(tree.node_mut(i), sheet);
    }

    #[cfg(debug_assertions)]
    for (i, w) in witness.iter().enumerate() {
        if !range.contains(&i) {
            debug_assert_eq!(
                out[i], *w,
                "cascade_subtree({root}) left node {i} disagreeing with a full cascade: the \
                 sheet changed a rule that matches outside the subtree"
            );
        }
    }
}

/// Cascade a tree *and every off-table arena under it*.
///
/// [`cascade`] walks one table, which is every host's entry point — and
/// every construct that keeps children off the table was therefore
/// unreachable by a stylesheet. A rule could not set the padding of a
/// repeater's row, and an `#[widget_id]` inside a template was dead for
/// styling as well as for agent selectors. Only the repeater's case was
/// documented as a known limit; `Either`, `List`, `Dyn` and the enum
/// derive inherited it silently, which is the difference between a
/// decision and an accident (`docs/plan-review-fixes.md` F4.5).
///
/// Recursion is by *template*: an arena's instances share one style row,
/// so a rule matching inside a template applies to every instance of it.
/// Per-instance styling would need per-instance rows and a selector
/// vocabulary that can name a row; neither exists, and this does not
/// pretend otherwise.
pub fn cascade_deep<W: crate::widget::Widget + ?Sized>(
    tree: &mut W,
    sheet: Stylesheet,
    out: &mut [Style],
) {
    {
        let t: &W = tree;
        cascade(W::SUB_TABLE, &|i| t.node(i).defaults(), &|i| t.node(i).kind_name(), sheet, out);
    }
    for i in 0..W::SUB_TABLE.len() {
        cascade_arenas(tree.node_mut(i), sheet);
    }
}

/// Cascade every arena `node` owns, and every arena under those.
fn cascade_arenas(node: &mut dyn crate::widget::Node, sheet: Stylesheet) {
    for a in 0..node.arena_count() {
        let Some(arena) = node.arena_mut(a) else { continue };
        let table = arena.table;
        let template = arena.template;
        cascade(
            table,
            &|i| template.node(i).defaults(),
            &|i| template.node(i).kind_name(),
            sheet,
            arena.style,
        );
    }
    // Arenas nested inside an arena's template are not reached: the
    // template is borrowed shared while its style row is borrowed
    // exclusively, so descending needs a second pass the borrow checker
    // will not give from here. A `List<Repeat<_>>` therefore styles the
    // list's rows but not the repeater's. Recorded rather than papered
    // over; the shape that fixes it is an arena walk that yields owned
    // handles, which is F4.4's territory.
}
