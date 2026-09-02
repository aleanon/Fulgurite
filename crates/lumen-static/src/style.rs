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
    debug_assert_eq!(table.len(), out.len());
    for (i, d) in table.iter().enumerate() {
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
